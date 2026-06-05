pub mod config;

use rand::Rng;
use std::any::Any;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context as _;
use onebot_v11::Event;
use onebot_v11::MessageSegment;
use onebot_v11::api::payload::ApiPayload;
use onebot_v11::api::payload::SendGroupMsg;
use onebot_v11::api::payload::SendPrivateMsg;
use onebot_v11::connect::ws_reverse::ReverseWsConnect;
use onebot_v11::event::message::GroupMessage;
use onebot_v11::event::message::Message as OneBotMessage;
use time::OffsetDateTime;
use time::UtcOffset;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc;

use crate::bot::message::BotCommand;
use crate::bot::message::ChannelMessage;
use crate::bot::message::ImageData;
use crate::bot::message::MessageActor;
use crate::bot::message::MessageContent;
use crate::bot::message::MessagePart;
use crate::bot::message::ReplyTarget;
use crate::bot::server::config::ReverseWebSockServerConfig;
use crate::bot::state::BotState;

const FIRST_REPLY_DELAY_MS: std::ops::Range<u64> = 2000..5000;
const BATCH_REPLY_DELAY_MS: std::ops::Range<u64> = 2000..3000;

type CommandFuture<'a> = Pin<Box<dyn Future<Output = Vec<BotCommand>> + 'a>>;
type NotificationPayload = Box<dyn Any + Send>;
type ChannelMessageHandlerObject = Box<dyn ChannelMessageHandler>;
type NotificationRegistrationObject = Box<dyn NotificationRegistration>;
type NotificationDispatcherObject = Box<dyn NotificationDispatcher>;

trait ChannelMessageHandler: Send {
    fn call<'a>(&'a self, state: &'a mut BotState, message: ChannelMessage) -> CommandFuture<'a>;
}

impl<F> ChannelMessageHandler for F
where
    F: for<'a> AsyncFn(&'a mut BotState, ChannelMessage) -> Vec<BotCommand> + Send,
{
    fn call<'a>(&'a self, state: &'a mut BotState, message: ChannelMessage) -> CommandFuture<'a> {
        Box::pin(self(state, message))
    }
}

struct NotificationEvent {
    registration_index: usize,
    payload: NotificationPayload,
}

trait NotificationDispatcher {
    fn dispatch<'a>(
        &'a self,
        state: &'a mut BotState,
        payload: NotificationPayload,
    ) -> CommandFuture<'a>;
}

struct TypedNotificationDispatcher<N, H> {
    handler: H,
    notification: PhantomData<fn(N)>,
}

impl<N, H> NotificationDispatcher for TypedNotificationDispatcher<N, H>
where
    N: Send + 'static,
    H: for<'a> AsyncFn(&'a mut BotState, N) -> Vec<BotCommand> + Send,
{
    fn dispatch<'a>(
        &'a self,
        state: &'a mut BotState,
        payload: NotificationPayload,
    ) -> CommandFuture<'a> {
        let notification = payload
            .downcast::<N>()
            .expect("notification payload type must match registration");
        Box::pin((self.handler)(state, *notification))
    }
}

trait NotificationRegistration: Send {
    fn spawn(
        self: Box<Self>,
        index: usize,
        send: mpsc::Sender<NotificationEvent>,
    ) -> anyhow::Result<()>;
}

struct TypedNotificationRegistration<N, S> {
    source: S,
    notification: PhantomData<fn(N)>,
}

impl<N, S> NotificationRegistration for TypedNotificationRegistration<N, S>
where
    N: Send + 'static,
    S: FnOnce() -> anyhow::Result<mpsc::Receiver<N>> + Send + 'static,
{
    fn spawn(
        self: Box<Self>,
        index: usize,
        send: mpsc::Sender<NotificationEvent>,
    ) -> anyhow::Result<()> {
        let Self { source, .. } = *self;
        let mut recv = source()?;

        tokio::spawn(async move {
            while let Some(notification) = recv.recv().await {
                let event = NotificationEvent {
                    registration_index: index,
                    payload: Box::new(notification),
                };

                if send.send(event).await.is_err() {
                    tracing::warn!("notification event bus dropped, source forwarder exiting");
                    break;
                }
            }
        });

        Ok(())
    }
}

pub struct BotServer {
    state: BotState,
    onebot_config: Option<ReverseWebSockServerConfig>,
    channel_message_handler: Option<ChannelMessageHandlerObject>,
    notification_registrations: Vec<NotificationRegistrationObject>,
    notification_dispatchers: Vec<NotificationDispatcherObject>,
}

impl BotServer {
    pub fn new(state: BotState) -> Self {
        Self {
            state,
            onebot_config: None,
            channel_message_handler: None,
            notification_registrations: Vec::new(),
            notification_dispatchers: Vec::new(),
        }
    }

    pub fn with_onebot_from_env(self) -> anyhow::Result<Self> {
        Ok(self.with_onebot(ReverseWebSockServerConfig::from_env()?))
    }

    pub fn with_onebot(mut self, config: ReverseWebSockServerConfig) -> Self {
        self.onebot_config = Some(config);
        self
    }

    pub fn on_channel_message<F>(mut self, handler: F) -> Self
    where
        F: for<'a> AsyncFn(&'a mut BotState, ChannelMessage) -> Vec<BotCommand> + Send + 'static,
    {
        self.channel_message_handler = Some(Box::new(handler));
        self
    }

    pub fn on_notification<N, S, H>(mut self, source: S, handler: H) -> Self
    where
        N: Send + 'static,
        S: FnOnce() -> anyhow::Result<mpsc::Receiver<N>> + Send + 'static,
        H: for<'a> AsyncFn(&'a mut BotState, N) -> Vec<BotCommand> + Send + 'static,
    {
        self.notification_registrations
            .push(Box::new(TypedNotificationRegistration {
                source,
                notification: PhantomData,
            }));
        self.notification_dispatchers
            .push(Box::new(TypedNotificationDispatcher {
                handler,
                notification: PhantomData,
            }));

        self
    }

    pub async fn serve(mut self) -> anyhow::Result<()> {
        let config = self
            .onebot_config
            .take()
            .context("BotServer is missing OneBot configuration")?;

        let conn = ReverseWsConnect::new(config.into()).await?;
        let command_sender = OneBotCommandSender::new(conn.clone());

        let mut event_recv = conn.subscribe().await;
        let (notification_send, mut notification_recv) = mpsc::channel::<NotificationEvent>(32);

        for (index, registration) in self.notification_registrations.drain(..).enumerate() {
            registration.spawn(index, notification_send.clone())?;
        }
        drop(notification_send);

        let mut notifications_closed = false;
        loop {
            tokio::select! {
                event = event_recv.recv() => {
                    self.handle_onebot_event(event, &command_sender).await;
                }
                notification = notification_recv.recv(), if !notifications_closed => {
                    match notification {
                        Some(notification) => {
                            let Some(dispatcher) = self.notification_dispatchers.get(notification.registration_index) else {
                                tracing::warn!(
                                    "missing notification dispatcher for index {}",
                                    notification.registration_index
                                );
                                continue;
                            };

                            let commands = dispatcher
                                .dispatch(&mut self.state, notification.payload)
                                .await;
                            command_sender.send_batch(commands, false).await;
                        }
                        None => {
                            notifications_closed = true;
                            tracing::warn!("all notification sources stopped");
                        }
                    }
                }
            }
        }
    }

    async fn handle_onebot_event(
        &mut self,
        event: Result<Event, RecvError>,
        command_sender: &OneBotCommandSender,
    ) {
        let Some(message) = receive_event(event).and_then(extract_channel_message) else {
            return;
        };

        let Some(handler) = self.channel_message_handler.as_ref() else {
            return;
        };

        let commands = handler.call(&mut self.state, message).await;
        command_sender.send_batch(commands, true).await;
    }
}

struct OneBotCommandSender {
    conn: Arc<ReverseWsConnect>,
}

impl OneBotCommandSender {
    fn new(conn: Arc<ReverseWsConnect>) -> Self {
        Self { conn }
    }

    async fn send_batch(&self, commands: Vec<BotCommand>, delayed: bool) {
        if commands.is_empty() {
            return;
        }

        if delayed {
            sleep_random(FIRST_REPLY_DELAY_MS).await;
        }

        let total = commands.len();
        for (i, command) in commands.into_iter().enumerate() {
            if let Err(error) = self.send_one(command).await {
                tracing::error!("failed to send bot command: {}", error);
                break;
            }

            if delayed && i + 1 < total {
                sleep_random(BATCH_REPLY_DELAY_MS).await;
            }
        }
    }

    async fn send_one(&self, command: BotCommand) -> anyhow::Result<()> {
        match command {
            BotCommand::ReplyTo { target, content } => self.send_reply(target, content).await,
            BotCommand::SendChannel {
                channel_id,
                content,
            } => self.send_channel(channel_id, content).await,
            BotCommand::SendDirect { actor_id, content } => {
                self.send_direct(actor_id, content).await
            }
        }
    }

    async fn send_reply(&self, target: ReplyTarget, content: MessageContent) -> anyhow::Result<()> {
        if content.is_empty() {
            return Ok(());
        }

        let mut message = Vec::with_capacity(content.len() + 1);
        message.push(MessageSegment::reply(target.message_id));
        message.extend(content_into_segments(content));

        self.send_channel_segments(target.channel_id, message).await
    }

    async fn send_channel(
        &self,
        channel_id: String,
        content: MessageContent,
    ) -> anyhow::Result<()> {
        if content.is_empty() {
            return Ok(());
        }

        self.send_channel_segments(channel_id, content_into_segments(content))
            .await
    }

    async fn send_direct(&self, actor_id: String, content: MessageContent) -> anyhow::Result<()> {
        if content.is_empty() {
            return Ok(());
        }

        let user_id = actor_id
            .parse::<i64>()
            .context("actor id must be a valid OneBot user id")?;
        let payload = ApiPayload::SendPrivateMsg(SendPrivateMsg {
            user_id,
            message: content_into_segments(content),
            auto_escape: false,
        });

        self.conn.clone().call_api(payload).await?;

        Ok(())
    }

    async fn send_channel_segments(
        &self,
        channel_id: String,
        message: Vec<MessageSegment>,
    ) -> anyhow::Result<()> {
        let group_id = channel_id
            .parse::<i64>()
            .context("channel id must be a valid OneBot group id")?;
        let payload = ApiPayload::SendGroupMsg(SendGroupMsg {
            group_id,
            message,
            auto_escape: false,
        });

        self.conn.clone().call_api(payload).await?;

        Ok(())
    }
}

fn receive_event(event: Result<Event, RecvError>) -> Option<Event> {
    if let Err(e) = &event {
        tracing::error!("failed to receive event: {}", e);
        return None;
    }

    event.ok()
}

fn extract_channel_message(event: Event) -> Option<ChannelMessage> {
    match event {
        Event::Message(OneBotMessage::GroupMessage(group_message)) => {
            Some(channel_message_from_onebot_group(group_message))
        }
        _ => None,
    }
}

fn channel_message_from_onebot_group(message: GroupMessage) -> ChannelMessage {
    ChannelMessage {
        self_id: message.self_id.to_string(),
        message_id: message.message_id.to_string(),
        channel_id: message.group_id.to_string(),
        actor: MessageActor {
            id: message.user_id.to_string(),
            nickname: message.sender.nickname.unwrap_or_default(),
            channel_nickname: message.sender.card,
        },
        sent_at: OffsetDateTime::from_unix_timestamp(message.time)
            .ok()
            .map(to_local_time)
            .unwrap_or_else(current_local_time),
        raw_text: message.raw_message,
        content: content_from_segments(message.message),
    }
}

fn content_from_segments(segments: Vec<MessageSegment>) -> MessageContent {
    MessageContent {
        parts: segments.into_iter().map(part_from_segment).collect(),
    }
}

fn part_from_segment(segment: MessageSegment) -> MessagePart {
    match segment {
        MessageSegment::Text { data } => MessagePart::Text(data.text),
        MessageSegment::At { data } => MessagePart::Mention { actor_id: data.qq },
        MessageSegment::Reply { data } => MessagePart::Reply {
            message_id: data.id,
        },
        _ => MessagePart::Other,
    }
}

fn content_into_segments(content: MessageContent) -> Vec<MessageSegment> {
    content
        .parts
        .into_iter()
        .filter_map(part_into_segment)
        .collect()
}

fn part_into_segment(part: MessagePart) -> Option<MessageSegment> {
    match part {
        MessagePart::Text(text) => Some(MessageSegment::text(text)),
        MessagePart::Mention { actor_id } => Some(MessageSegment::at(actor_id)),
        MessagePart::Reply { message_id } => Some(MessageSegment::reply(message_id)),
        MessagePart::Image {
            data: ImageData::Base64(image_base64),
        } => Some(MessageSegment::easy_image(
            format!("base64://{}", image_base64),
            None::<String>,
        )),
        MessagePart::Other => None,
    }
}

fn to_local_time(time: OffsetDateTime) -> OffsetDateTime {
    let local_offset = UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC);
    time.to_offset(local_offset)
}

fn current_local_time() -> OffsetDateTime {
    let local_offset = UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC);
    OffsetDateTime::now_utc().to_offset(local_offset)
}

async fn sleep_random(range: std::ops::Range<u64>) {
    let delay_ms = rand::thread_rng().gen_range(range);
    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bot::agent::BotAgent;
    use onebot_v11::event::message::GroupMessageSender;

    async fn notification_handler(_: &mut BotState, value: String) -> Vec<BotCommand> {
        vec![BotCommand::channel_text("200", value)]
    }

    async fn channel_handler(_: &mut BotState, message: ChannelMessage) -> Vec<BotCommand> {
        vec![BotCommand::channel_text(message.channel_id, "ok")]
    }

    fn test_state() -> BotState {
        BotState::new(BotAgent::new_for_test(), "100")
    }

    #[test]
    fn onebot_group_message_maps_to_channel_message() {
        let message = GroupMessage {
            time: 1_700_000_000,
            self_id: 100,
            post_type: "message".to_string(),
            message_type: "group".to_string(),
            sub_type: "normal".to_string(),
            message_id: 10,
            group_id: 200,
            user_id: 300,
            anonymous: None,
            message: vec![
                MessageSegment::reply("9"),
                MessageSegment::at("100"),
                MessageSegment::text(" hello"),
            ],
            raw_message: "[CQ:at,qq=100] hello".to_string(),
            font: 0,
            sender: GroupMessageSender {
                user_id: Some(300),
                nickname: Some("Alice".to_string()),
                card: Some("A".to_string()),
                sex: None,
                age: None,
                area: None,
                level: None,
                role: None,
                title: None,
            },
        };

        let mapped = channel_message_from_onebot_group(message);

        assert_eq!(mapped.self_id, "100");
        assert_eq!(mapped.message_id, "10");
        assert_eq!(mapped.channel_id, "200");
        assert_eq!(mapped.actor.id, "300");
        assert_eq!(mapped.actor.nickname, "Alice");
        assert_eq!(mapped.actor.channel_nickname.as_deref(), Some("A"));
        assert_eq!(
            mapped.content.parts,
            vec![
                MessagePart::Reply {
                    message_id: "9".to_string()
                },
                MessagePart::Mention {
                    actor_id: "100".to_string()
                },
                MessagePart::Text(" hello".to_string())
            ]
        );
    }

    #[test]
    fn image_part_maps_to_onebot_base64_image() {
        let segments = content_into_segments(MessageContent {
            parts: vec![MessagePart::Image {
                data: ImageData::Base64("abc".to_string()),
            }],
        });

        match &segments[0] {
            MessageSegment::Image { data } => assert_eq!(data.file, "base64://abc"),
            other => panic!("unexpected segment: {:?}", other),
        }
    }

    #[tokio::test]
    async fn notification_dispatcher_dispatches_registered_handler() {
        let dispatcher = TypedNotificationDispatcher {
            handler: notification_handler,
            notification: PhantomData,
        };
        let mut state = test_state();

        let commands = dispatcher
            .dispatch(&mut state, Box::new("hello".to_string()))
            .await;

        assert_eq!(commands, vec![BotCommand::channel_text("200", "hello")]);
    }

    #[tokio::test]
    async fn channel_handler_adapter_accepts_async_fn() {
        let handler: ChannelMessageHandlerObject = Box::new(channel_handler);
        let mut state = test_state();
        let message = ChannelMessage {
            self_id: "100".to_string(),
            message_id: "1".to_string(),
            channel_id: "200".to_string(),
            actor: MessageActor {
                id: "300".to_string(),
                nickname: "Alice".to_string(),
                channel_nickname: None,
            },
            sent_at: current_local_time(),
            raw_text: "hello".to_string(),
            content: MessageContent::text("hello"),
        };

        let commands = handler.call(&mut state, message).await;

        assert_eq!(commands, vec![BotCommand::channel_text("200", "ok")]);
    }
}
