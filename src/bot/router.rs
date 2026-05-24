use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::bot::message::Message;
use crate::bot::state::BotState;

type GroupMessageFuture = Pin<Box<dyn Future<Output = anyhow::Result<Option<Message>>> + Send>>;
type GroupMessageHandler =
    Arc<dyn Fn(BotState, Message) -> GroupMessageFuture + Send + Sync + 'static>;

#[derive(Clone, Default)]
pub struct Router {
    group_message_handlers: Vec<GroupMessageHandler>,
}

impl Router {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn on_group_message<F, Fut>(mut self, handler: F) -> Self
    where
        F: Fn(BotState, Message) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = anyhow::Result<Option<Message>>> + Send + 'static,
    {
        self.group_message_handlers
            .push(Arc::new(move |state, msg| Box::pin(handler(state, msg))));

        self
    }

    pub(crate) async fn handle_group_message(
        &self,
        state: BotState,
        message: Message,
    ) -> anyhow::Result<Option<Message>> {
        for handler in &self.group_message_handlers {
            if let Some(reply) = handler(state.clone(), message.clone()).await? {
                return Ok(Some(reply));
            }
        }

        Ok(None)
    }
}
