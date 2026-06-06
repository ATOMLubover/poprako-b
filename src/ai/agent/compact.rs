use std::marker::PhantomData;

use async_trait::async_trait;

use crate::ai::resolver::context::Context;
use crate::ai::resolver::message::{IMessage, MessageRole};

#[async_trait]
pub trait ICompact: Send {
    type Message: IMessage + Send + Sync + 'static;
    type State: Send + Sync + 'static;
    type Annotation: Default + Send + Sync + 'static;

    async fn compact(
        &mut self,
        state: &mut Self::State,
        cx: &mut Context<Self::Message, Self::Annotation>,
    );
}

pub type DynCompact<M, S = (), A = ()> = Box<dyn ICompact<Message = M, State = S, Annotation = A>>;

pub struct SlidingWindowCompact<M, S = (), A = ()> {
    max_messages: usize,
    reserve_messages: usize,
    #[allow(clippy::type_complexity)]
    marker: PhantomData<fn() -> (M, S, A)>,
}

impl<M, S, A> SlidingWindowCompact<M, S, A> {
    pub fn new(max_messages: usize, reserve_messages: usize) -> Self {
        Self {
            max_messages,
            reserve_messages,
            marker: PhantomData,
        }
    }
}

impl<M, S, A> Default for SlidingWindowCompact<M, S, A> {
    fn default() -> Self {
        Self::new(80, 50)
    }
}

#[async_trait]
impl<M, S, A> ICompact for SlidingWindowCompact<M, S, A>
where
    M: IMessage + Send + Sync + 'static,
    S: Send + Sync + 'static,
    A: Default + Send + Sync + 'static,
{
    type Message = M;
    type State = S;
    type Annotation = A;

    async fn compact(&mut self, _state: &mut S, cx: &mut Context<M, A>) {
        let len = if let len = cx.message_count()
            && len > self.max_messages
        {
            len
        } else {
            return;
        };

        let mut messages = cx.take_annotated_messages();

        messages.drain(1..len.saturating_sub(self.reserve_messages));

        let user_first = messages
            .iter()
            .skip(1)
            .position(|m| m.message.role() == MessageRole::User)
            .map(|i| i + 1);

        if let Some(i) = user_first
            && i > 1
        {
            messages.drain(1..i);
        }

        cx.set_annotated_messages(messages);
    }
}
