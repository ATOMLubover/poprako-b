use async_trait::async_trait;

use crate::ai::agent::interceptor::IInterceptor;
use crate::ai::agent::interceptor::InterceptorFlow;
use crate::ai::resolver::context::AnnotatedMessage;
use crate::ai::resolver::context::Context;
use crate::ai::resolver::message::IMessage;
use crate::ai::resolver::message::MessageOwned;
use crate::ai::resolver::message::MessageRef;
use crate::bot::agent::plugin::inspiration::annotation::IInspirationAnnotated;
use crate::bot::agent::plugin::inspiration::annotation::InspiredAnnotation;
use crate::bot::agent::plugin::inspiration::input::MatchInput;
use crate::bot::agent::plugin::inspiration::knowledge::KnowledgeEntry;
use crate::bot::agent::plugin::inspiration::knowledge::KnowledgeRegistry;
use crate::bot::agent::plugin::inspiration::state::IInspirationEmbedded;
use crate::bot::agent::plugin::inspiration::state::InspiredState;

fn last_user_text<M, A>(cx: &Context<M, A>) -> Option<&str>
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
    state: &mut InspiredState,
    entry: KnowledgeEntry,
) where
    M: IMessage + Send + Sync + 'static,
    A: IInspirationAnnotated + Default,
{
    let content = format!(
        "[灵光一闪][{}][{}] {}",
        entry.namespace, entry.title, entry.content
    );
    let message = M::from(MessageOwned::User { content });

    let mut annotation = A::default();
    *annotation.inspired_annotation_mut() = InspiredAnnotation::with_knowledge_id(entry.id.clone());

    cx.inject_before_last(AnnotatedMessage::new(message, annotation));
    state.active_knowledge_ids.insert(entry.id);
}

pub struct InspirationInterceptor<M, S, A> {
    knowledge_registry: KnowledgeRegistry,
    #[allow(clippy::complexity)]
    marker: std::marker::PhantomData<fn() -> (M, S, A)>,
}

impl<M, S, A> InspirationInterceptor<M, S, A> {
    pub fn new(knowledge_registry: KnowledgeRegistry) -> Self {
        Self {
            knowledge_registry,
            marker: std::marker::PhantomData,
        }
    }
}

#[async_trait]
impl<M, S, A> IInterceptor<S, M, A> for InspirationInterceptor<M, S, A>
where
    M: IMessage + Send + Sync + 'static,
    S: IInspirationEmbedded + Send + Sync + 'static,
    A: IInspirationAnnotated + Default + Send + Sync + 'static,
{
    async fn before_solve(&mut self, state: &mut S, cx: &mut Context<M, A>) -> InterceptorFlow {
        let Some(user_text) = last_user_text(cx) else {
            return InterceptorFlow::Continue;
        };

        let input = MatchInput::parse(user_text);

        let inspiration_state = state.inspired_state_mut();
        let entries = self
            .knowledge_registry
            .match_entries(&input, inspiration_state);

        for entry in entries {
            inject_inspiration(cx, inspiration_state, entry);
        }

        InterceptorFlow::Continue
    }
}
