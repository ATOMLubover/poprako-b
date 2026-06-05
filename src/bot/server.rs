pub mod config;
mod handler;
mod onebot;

use anyhow::Context as _;
use onebot_v11::Event;
use onebot_v11::connect::ws_reverse::ReverseWsConnect;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc;

use crate::bot::message::BotCommand;
use crate::bot::message::ChannelMessage;
use crate::bot::server::config::ReverseWebSockServerConfig;
use crate::bot::server::handler::ChannelHandlerBox;
use crate::bot::server::handler::Notice;
use crate::bot::server::handler::NoticeHandlerBox;
use crate::bot::server::handler::WatchBox;
use crate::bot::server::handler::WatchPair;
use crate::bot::server::onebot::OneBotSender;
use crate::bot::server::onebot::channel_message_from_event;
use crate::bot::state::BotState;

pub struct BotServer {
    state: BotState,
    onebot_config: Option<ReverseWebSockServerConfig>,
    channel_handler: Option<ChannelHandlerBox>,
    watches: Vec<WatchBox>,
    notice_handlers: Vec<NoticeHandlerBox>,
}

impl BotServer {
    pub fn new(state: BotState) -> Self {
        Self {
            state,
            onebot_config: None,
            channel_handler: None,
            watches: Vec::new(),
            notice_handlers: Vec::new(),
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
        self.channel_handler = Some(Box::new(handler));
        self
    }

    pub fn on_notice<N, S, H>(mut self, source: S, handler: H) -> Self
    where
        N: Send + 'static,
        S: FnOnce() -> anyhow::Result<mpsc::Receiver<N>> + Send + 'static,
        H: for<'a> AsyncFn(&'a mut BotState, N) -> Vec<BotCommand> + Send + 'static,
    {
        let pair = WatchPair::new(source, handler);
        self.watches.push(pair.watch);
        self.notice_handlers.push(pair.handler);
        self
    }

    pub async fn serve(mut self) -> anyhow::Result<()> {
        let config = self
            .onebot_config
            .take()
            .context("BotServer is missing OneBot configuration")?;

        let connect = ReverseWsConnect::new(config.into()).await?;
        let sender = OneBotSender::new(connect.clone());
        let mut event_recv = connect.subscribe().await;
        let (notice_send, mut notice_recv) = mpsc::channel::<Notice>(32);

        for (index, watch) in self.watches.drain(..).enumerate() {
            watch.spawn(index, notice_send.clone())?;
        }

        drop(notice_send);

        let mut notices_closed = false;
        loop {
            tokio::select! {
                event = event_recv.recv() => {
                    self.handle_onebot_event(event, &sender).await;
                }
                notice = notice_recv.recv(), if !notices_closed => {
                    match notice {
                        Some(notice) => self.handle_notice(notice, &sender).await,
                        None => {
                            notices_closed = true;
                            tracing::warn!("all notice sources stopped");
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

        let Some(handler) = self.channel_handler.as_ref() else {
            return;
        };

        let commands = handler.call(&mut self.state, message).await;
        sender.send_batch(commands, true).await;
    }

    async fn handle_notice(&mut self, notice: Notice, sender: &OneBotSender) {
        let Some(handler) = self.notice_handlers.get(notice.index) else {
            tracing::warn!("missing notice handler for index {}", notice.index);
            return;
        };

        let commands = handler.call(&mut self.state, notice.body).await;
        sender.send_batch(commands, false).await;
    }
}
