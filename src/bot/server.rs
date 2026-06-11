pub mod config;
mod onebot;

use anyhow::Context as _;
use onebot_v11::Event;
use onebot_v11::connect::ws_reverse::ReverseWsConnect;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc;

use crate::bot::app::BotApp;
use crate::bot::event::BotEvent;
use crate::bot::server::config::ReverseWebSockServerConfig;
use crate::bot::server::onebot::{OneBotSender, channel_message_from_event};

trait EventSource: Send {
    fn spawn(self: Box<Self>, send: mpsc::Sender<BotEvent>) -> anyhow::Result<()>;
}

type EventSourceBox = Box<dyn EventSource>;

struct ReceiverEventSource<N, S, F> {
    source: S,
    map: F,
    kind: std::marker::PhantomData<fn(N)>,
}

impl<N, S, F> ReceiverEventSource<N, S, F> {
    fn new(source: S, map: F) -> Self {
        Self {
            source,
            map,
            kind: std::marker::PhantomData,
        }
    }
}

impl<N, S, F> EventSource for ReceiverEventSource<N, S, F>
where
    N: Send + 'static,
    S: FnOnce() -> anyhow::Result<mpsc::Receiver<N>> + Send + 'static,
    F: Fn(N) -> BotEvent + Send + 'static,
{
    fn spawn(self: Box<Self>, send: mpsc::Sender<BotEvent>) -> anyhow::Result<()> {
        let Self { source, map, .. } = *self;
        let mut recv = source()?;

        tokio::spawn(async move {
            while let Some(body) = recv.recv().await {
                if send.send(map(body)).await.is_err() {
                    tracing::warn!("bot event bus dropped, event source forwarder exiting");
                    break;
                }
            }
        });

        Ok(())
    }
}

pub struct BotServer {
    app: BotApp,
    onebot_config: Option<ReverseWebSockServerConfig>,
    event_sources: Vec<EventSourceBox>,
}

impl BotServer {
    pub fn new(app: BotApp) -> Self {
        Self {
            app,
            onebot_config: None,
            event_sources: Vec::new(),
        }
    }

    pub fn with_onebot_from_env(self) -> anyhow::Result<Self> {
        Ok(self.with_onebot(ReverseWebSockServerConfig::from_env()?))
    }

    pub fn with_onebot(mut self, config: ReverseWebSockServerConfig) -> Self {
        self.onebot_config = Some(config);
        self
    }

    pub fn on_event_source<N, S, F>(mut self, source: S, map: F) -> Self
    where
        N: Send + 'static,
        S: FnOnce() -> anyhow::Result<mpsc::Receiver<N>> + Send + 'static,
        F: Fn(N) -> BotEvent + Send + 'static,
    {
        self.event_sources
            .push(Box::new(ReceiverEventSource::new(source, map)));
        self
    }

    pub async fn serve(mut self) -> anyhow::Result<()> {
        let config = self
            .onebot_config
            .take()
            .context("BotServer is missing OneBot configuration")?;

        let connect = ReverseWsConnect::new(config.into()).await?;
        let sender = OneBotSender::new(connect.clone());
        let mut onebot_event_recv = connect.subscribe().await;

        let (source_event_send, mut source_event_recv) = mpsc::channel::<BotEvent>(32);

        for source in self.event_sources.drain(..) {
            source.spawn(source_event_send.clone())?;
        }

        drop(source_event_send);

        let mut sources_closed = false;
        loop {
            tokio::select! {
                event = onebot_event_recv.recv() => {
                    self.handle_onebot_event(event, &sender).await;
                }
                event = source_event_recv.recv(), if !sources_closed => {
                    match event {
                        Some(event) => self.handle_bot_event(event, &sender).await,
                        None => {
                            sources_closed = true;
                            tracing::warn!("all bot event sources stopped");
                        }
                    }
                }
            }
        }
    }

    async fn handle_onebot_event(
        &mut self,
        event: Result<Event, RecvError>,
        sender: &OneBotSender,
    ) {
        let Some(message) = channel_message_from_event(event) else {
            return;
        };

        self.handle_bot_event(BotEvent::ChannelMessage(message), sender)
            .await;
    }

    async fn handle_bot_event(&mut self, event: BotEvent, sender: &OneBotSender) {
        let delayed = event.should_delay_response();
        let commands = self.app.handle(event).await;

        sender.send_batch(commands, delayed).await;
    }
}
