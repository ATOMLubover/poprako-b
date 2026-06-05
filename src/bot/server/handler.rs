use std::any::Any;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;

use tokio::sync::mpsc;

use crate::bot::message::BotCommand;
use crate::bot::message::ChannelMessage;
use crate::bot::state::BotState;

type CommandFuture<'a> = Pin<Box<dyn Future<Output = Vec<BotCommand>> + 'a>>;
type NoticeBody = Box<dyn Any + Send>;

pub type ChannelHandlerBox = Box<dyn ChannelHandler>;
pub type WatchBox = Box<dyn Watch>;
pub type NoticeHandlerBox = Box<dyn NoticeHandler>;

pub trait ChannelHandler: Send {
    fn call<'a>(&'a self, state: &'a mut BotState, message: ChannelMessage) -> CommandFuture<'a>;
}

impl<F> ChannelHandler for F
where
    F: for<'a> AsyncFn(&'a mut BotState, ChannelMessage) -> Vec<BotCommand> + Send,
{
    fn call<'a>(&'a self, state: &'a mut BotState, message: ChannelMessage) -> CommandFuture<'a> {
        Box::pin(self(state, message))
    }
}

pub struct Notice {
    pub index: usize,
    pub body: NoticeBody,
}

pub trait NoticeHandler {
    fn call<'a>(&'a self, state: &'a mut BotState, body: NoticeBody) -> CommandFuture<'a>;
}

pub struct NoticeCall<N, H> {
    handler: H,
    kind: PhantomData<fn(N)>,
}

impl<N, H> NoticeCall<N, H> {
    fn new(handler: H) -> Self {
        Self {
            handler,
            kind: PhantomData,
        }
    }
}

impl<N, H> NoticeHandler for NoticeCall<N, H>
where
    N: Send + 'static,
    H: for<'a> AsyncFn(&'a mut BotState, N) -> Vec<BotCommand> + Send,
{
    fn call<'a>(&'a self, state: &'a mut BotState, body: NoticeBody) -> CommandFuture<'a> {
        let body = body
            .downcast::<N>()
            .expect("notice body type must match watch");
        Box::pin((self.handler)(state, *body))
    }
}

pub trait Watch: Send {
    fn spawn(self: Box<Self>, index: usize, send: mpsc::Sender<Notice>) -> anyhow::Result<()>;
}

struct NoticeWatch<N, S> {
    source: S,
    kind: PhantomData<fn(N)>,
}

impl<N, S> Watch for NoticeWatch<N, S>
where
    N: Send + 'static,
    S: FnOnce() -> anyhow::Result<mpsc::Receiver<N>> + Send + 'static,
{
    fn spawn(self: Box<Self>, index: usize, send: mpsc::Sender<Notice>) -> anyhow::Result<()> {
        let Self { source, .. } = *self;
        let mut recv = source()?;

        tokio::spawn(async move {
            while let Some(body) = recv.recv().await {
                let notice = Notice {
                    index,
                    body: Box::new(body),
                };

                if send.send(notice).await.is_err() {
                    tracing::warn!("notice bus dropped, source forwarder exiting");
                    break;
                }
            }
        });

        Ok(())
    }
}

pub struct WatchPair {
    pub watch: WatchBox,
    pub handler: NoticeHandlerBox,
}

impl WatchPair {
    pub fn new<N, S, H>(source: S, handler: H) -> Self
    where
        N: Send + 'static,
        S: FnOnce() -> anyhow::Result<mpsc::Receiver<N>> + Send + 'static,
        H: for<'a> AsyncFn(&'a mut BotState, N) -> Vec<BotCommand> + Send + 'static,
    {
        Self {
            watch: Box::new(NoticeWatch {
                source,
                kind: PhantomData::<fn(N)>,
            }),
            handler: Box::new(NoticeCall::new(handler)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bot::agent::BotAgent;
    use crate::bot::message::MessageActor;
    use crate::bot::message::MessageContent;

    async fn notice_handler(_: &mut BotState, value: String) -> Vec<BotCommand> {
        vec![BotCommand::channel_text("200", value)]
    }

    async fn channel_handler(_: &mut BotState, message: ChannelMessage) -> Vec<BotCommand> {
        vec![BotCommand::channel_text(message.channel_id, "ok")]
    }

    fn test_state() -> BotState {
        BotState::new(BotAgent::new_for_test(), "100")
    }

    #[tokio::test]
    async fn notice_handler_dispatches_registered_handler() {
        let handler = NoticeCall::new(notice_handler);
        let mut state = test_state();

        let commands = handler
            .call(&mut state, Box::new("hello".to_string()))
            .await;

        assert_eq!(commands, vec![BotCommand::channel_text("200", "hello")]);
    }

    #[tokio::test]
    async fn channel_handler_accepts_async_fn() {
        let handler: ChannelHandlerBox = Box::new(channel_handler);
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
            sent_at: time::OffsetDateTime::now_utc(),
            raw_text: "hello".to_string(),
            content: MessageContent::text("hello"),
        };

        let commands = handler.call(&mut state, message).await;

        assert_eq!(commands, vec![BotCommand::channel_text("200", "ok")]);
    }
}
