use async_trait::async_trait;

use crate::ai::agent::interceptor::IInterceptor;
use crate::ai::agent::interceptor::InterceptorFlow;
use crate::ai::resolver::context::AnnotatedMessage;
use crate::ai::resolver::context::Context;
use crate::ai::resolver::message::IMessage;
use crate::ai::resolver::message::MessageOwned;
use crate::ai::resolver::message::MessageRef;
use crate::bot::agent::plugin::inspiration::annotation::IWithInspirationAnnotation;
use crate::bot::agent::plugin::inspiration::annotation::InspirationAnnotation;
use crate::bot::agent::plugin::inspiration::input::MatchInput;
use crate::bot::agent::plugin::inspiration::knowledge::InspirationKnowledge;
use crate::bot::agent::plugin::inspiration::knowledge::KnowledgeEntry;
use crate::bot::agent::plugin::inspiration::state::IWithInspirationState;
use crate::bot::agent::plugin::inspiration::state::InspirationState;

fn latest_user_text<M, A>(cx: &Context<M, A>) -> Option<&str>
where
    M: IMessage + Send + Sync + 'static,
{
    let message = cx.annotated_messages().last()?.message.message_ref();
    match message {
        MessageRef::User { content } => Some(content),
        _ => None,
    }
}

fn inject_inspiration<M, A>(
    cx: &mut Context<M, A>,
    state: &mut InspirationState,
    entry: KnowledgeEntry,
) where
    M: IMessage + Send + Sync + 'static,
    A: IWithInspirationAnnotation + Default,
{
    let content = format!("[灵光一闪]\n{}", entry.content);
    let message = M::from(MessageOwned::User { content });
    let mut annotation = A::default();
    *annotation.inspiration_annotation_mut() = InspirationAnnotation::inspiration(entry.id);

    cx.inject_before_last(AnnotatedMessage::new(message, annotation));
    state.active_inspiration_ids.insert(entry.id.to_string());
}

pub struct InspirationInterceptor<M, S, A> {
    knowledge: InspirationKnowledge,
    marker: std::marker::PhantomData<fn() -> (M, S, A)>,
}

impl<M, S, A> Default for InspirationInterceptor<M, S, A> {
    fn default() -> Self {
        Self {
            knowledge: InspirationKnowledge,
            marker: std::marker::PhantomData,
        }
    }
}

#[async_trait]
impl<M, S, A> IInterceptor<S, M, A> for InspirationInterceptor<M, S, A>
where
    M: IMessage + Send + Sync + 'static,
    S: IWithInspirationState + Send + Sync + 'static,
    A: IWithInspirationAnnotation + Default + Send + Sync + 'static,
{
    async fn before_solve(&mut self, state: &mut S, cx: &mut Context<M, A>) -> InterceptorFlow {
        let Some(user_text) = latest_user_text(cx) else {
            return InterceptorFlow::Continue;
        };

        let input = MatchInput::parse(user_text);
        let inspiration_state = state.inspiration_state_mut();
        let entries = self.knowledge.match_entries(&input, inspiration_state);

        for entry in entries {
            inject_inspiration(cx, inspiration_state, entry);
        }

        InterceptorFlow::Continue
    }
}
