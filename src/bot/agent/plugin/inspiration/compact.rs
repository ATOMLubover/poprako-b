use std::collections::HashSet;

use async_trait::async_trait;

use crate::ai::agent::compact::ICompact;
use crate::ai::agent::compact::SlidingWindowCompact;
use crate::ai::resolver::context::Context;
use crate::ai::resolver::message::IMessage;
use crate::bot::agent::plugin::inspiration::annotation::IInspirationAnnotated;
use crate::bot::agent::plugin::inspiration::state::IInspirationEmbedded;

fn retained_knowledge_ids<M, A>(cx: &Context<M, A>) -> HashSet<String>
where
    M: IMessage + Send + Sync + 'static,
    A: IInspirationAnnotated,
{
    cx.annotated_messages()
        .iter()
        .filter_map(|message| {
            message
                .annotation
                .inspired_annotation()
                .knowledge_id()
                .map(str::to_string)
        })
        .collect()
}

pub struct BotCompact<M, S, A> {
    inner: SlidingWindowCompact<M, S, A>,
}

impl<M, S, A> Default for BotCompact<M, S, A> {
    fn default() -> Self {
        Self {
            inner: SlidingWindowCompact::default(),
        }
    }
}

#[async_trait]
impl<M, S, A> ICompact for BotCompact<M, S, A>
where
    M: IMessage + Send + Sync + 'static,
    S: IInspirationEmbedded + Send + Sync + 'static,
    A: IInspirationAnnotated + Default + Send + Sync + 'static,
{
    type Message = M;
    type State = S;
    type Annotation = A;

    async fn compact(&mut self, state: &mut S, cx: &mut Context<Self::Message, Self::Annotation>) {
        self.inner.compact(state, cx).await;
        state.inspired_state_mut().active_knowledge_ids = retained_knowledge_ids(cx);
    }
}
