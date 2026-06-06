mod annotation;
mod compact;
mod input;
mod interceptor;
mod knowledge;
mod state;

use crate::ai::agent::AgentBuilder;
use crate::ai::agent::IAgentPlugin;
use crate::ai::resolver::IResolver;
use crate::ai::resolver::message::IMessage;
use crate::bot::agent::plugin::inspiration::compact::InspirationCompact;
use crate::bot::agent::plugin::inspiration::interceptor::InspirationInterceptor;

pub use crate::bot::agent::plugin::inspiration::annotation::IWithInspirationAnnotation;
pub use crate::bot::agent::plugin::inspiration::annotation::InspirationAnnotation;
pub use crate::bot::agent::plugin::inspiration::state::IWithInspirationState;
pub use crate::bot::agent::plugin::inspiration::state::InspirationState;

pub struct InspirationPlugin;

impl<M, R, S, A> IAgentPlugin<M, R, S, A> for InspirationPlugin
where
    M: IMessage + Send + Sync + 'static,
    R: IResolver<Message = M> + Send,
    S: IWithInspirationState + Send + Sync + 'static,
    A: IWithInspirationAnnotation + Default + Send + Sync + 'static,
{
    fn apply(&self, builder: AgentBuilder<M, R, S, A>) -> AgentBuilder<M, R, S, A> {
        builder
            .compact(InspirationCompact::<M, S, A>::default())
            .interceptor(InspirationInterceptor::<M, S, A>::default())
    }
}

pub fn plugin_inspiration() -> InspirationPlugin {
    InspirationPlugin
}

#[cfg(test)]
mod tests {
    use openai_oxide::types::chat::ChatCompletionMessageParam;

    use crate::ai::agent::compact::ICompact;
    use crate::ai::agent::interceptor::IInterceptor;
    use crate::ai::resolver::context::AnnotatedMessage;
    use crate::ai::resolver::context::ContextBuilder;
    use crate::ai::resolver::message::IMessage;
    use crate::ai::resolver::message::MessageRef;
    use crate::bot::agent::plugin::inspiration::annotation::InspirationAnnotation;
    use crate::bot::agent::plugin::inspiration::compact::InspirationCompact;
    use crate::bot::agent::plugin::inspiration::interceptor::InspirationInterceptor;
    use crate::bot::agent::plugin::inspiration::state::InspirationState;

    fn user(content: &str) -> ChatCompletionMessageParam {
        MessageRef::User { content }.into()
    }

    fn annotated_user(
        content: &str,
        annotation: InspirationAnnotation,
    ) -> AnnotatedMessage<ChatCompletionMessageParam, InspirationAnnotation> {
        AnnotatedMessage::new(user(content), annotation)
    }

    #[tokio::test]
    async fn before_solve_injects_matched_inspiration_before_latest_user_message() {
        let mut interceptor = InspirationInterceptor::<ChatCompletionMessageParam, _, _>::default();
        let mut state = InspirationState::default();
        let mut cx = ContextBuilder::<_, InspirationAnnotation>::new("test-model")
            .messages(vec![user(
                "[channel_id: 1, channel_name: -, sender_id: 2, sender_nickname: LB, sender_channel_nickname: -, sender_prks_id: -, sent_at: now]\n帮我看看",
            )])
            .build();

        interceptor.before_solve(&mut state, &mut cx).await;

        assert_eq!(cx.message_count(), 2);
        assert!(state.active_inspiration_ids.contains("member.lb"));
        match cx.annotated_messages()[1].message.message_ref() {
            MessageRef::User { content } => assert_eq!(
                content,
                "[channel_id: 1, channel_name: -, sender_id: 2, sender_nickname: LB, sender_channel_nickname: -, sender_prks_id: -, sent_at: now]\n帮我看看"
            ),
            _ => panic!("latest message should remain a user message"),
        }
        assert_eq!(
            cx.annotated_messages()[0].annotation.inspiration_id(),
            Some("member.lb")
        );
    }

    #[tokio::test]
    async fn before_solve_does_not_duplicate_active_inspiration() {
        let mut interceptor = InspirationInterceptor::<ChatCompletionMessageParam, _, _>::default();
        let mut state = InspirationState::default();
        state
            .active_inspiration_ids
            .insert("member.nabai".to_string());
        let mut cx = ContextBuilder::<_, InspirationAnnotation>::new("test-model")
            .messages(vec![user(
                "[channel_id: 1, channel_name: -, sender_id: 2, sender_nickname: 小明, sender_channel_nickname: -, sender_prks_id: -, sent_at: now]\n那白在吗",
            )])
            .build();

        interceptor.before_solve(&mut state, &mut cx).await;

        assert_eq!(cx.message_count(), 1);
    }

    #[tokio::test]
    async fn compact_rebuilds_state_from_retained_annotations() {
        let mut state = InspirationState::default();
        state.active_inspiration_ids.insert("member.lb".to_string());
        state
            .active_inspiration_ids
            .insert("member.nabai".to_string());
        let mut cx = ContextBuilder::<_, InspirationAnnotation>::new("test-model")
            .annotated_messages(vec![
                annotated_user(
                    "[灵光一闪]\n那白：翻译",
                    InspirationAnnotation::inspiration("member.nabai"),
                ),
                annotated_user("那白在吗", InspirationAnnotation::default()),
            ])
            .build();
        let mut compact = InspirationCompact::<ChatCompletionMessageParam, _, _>::default();

        compact.compact(&mut state, &mut cx).await;

        assert_eq!(state.active_inspiration_ids.len(), 1);
        assert!(state.active_inspiration_ids.contains("member.nabai"));
    }
}
