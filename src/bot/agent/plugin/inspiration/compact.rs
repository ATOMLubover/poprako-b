use std::collections::HashSet;

use async_trait::async_trait;

use crate::ai::agent::compact::ICompact;
use crate::ai::agent::compact::SlidingWindowCompact;
use crate::ai::resolver::context::Context;
use crate::ai::resolver::message::IMessage;
use crate::bot::agent::plugin::inspiration::annotation::IWithInspirationAnnotation;
use crate::bot::agent::plugin::inspiration::state::IWithInspirationState;

fn retained_inspiration_ids<M, A>(cx: &Context<M, A>) -> HashSet<String>
where
    M: IMessage + Send + Sync + 'static,
    A: IWithInspirationAnnotation,
{
    cx.annotated_messages()
        .iter()
        .filter_map(|message| {
            message
                .annotation
                .inspiration_annotation()
                .inspiration_id()
                .map(str::to_string)
        })
        .collect()
}

pub struct InspirationCompact<M, S, A> {
    inner: SlidingWindowCompact<M, S, A>,
}

impl<M, S, A> Default for InspirationCompact<M, S, A> {
    fn default() -> Self {
        Self {
            inner: SlidingWindowCompact::default(),
        }
    }
}

#[async_trait]
impl<M, S, A> ICompact for InspirationCompact<M, S, A>
where
    M: IMessage + Send + Sync + 'static,
    S: IWithInspirationState + Send + Sync + 'static,
    A: IWithInspirationAnnotation + Default + Send + Sync + 'static,
{
    type Message = M;
    type State = S;
    type Annotation = A;

    async fn compact(&mut self, state: &mut S, cx: &mut Context<Self::Message, Self::Annotation>) {
        self.inner.compact(state, cx).await;
        state.inspiration_state_mut().active_inspiration_ids = retained_inspiration_ids(cx);
    }
}
