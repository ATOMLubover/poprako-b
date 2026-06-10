mod annotation;
pub mod compact;
mod input;
mod interceptor;
mod knowledge;
mod state;

use crate::ai::agent::IAgentPlugin;
use crate::ai::agent::interceptor::DynInterceptor;
use crate::ai::resolver::IResolver;
use crate::ai::resolver::message::IMessage;
use crate::bot::agent::plugin::inspiration::interceptor::InspirationInterceptor;
use crate::bot::agent::plugin::inspiration::knowledge::KnowledgeRegistry;

pub use crate::bot::agent::plugin::inspiration::annotation::IInspirationAnnotated;
pub use crate::bot::agent::plugin::inspiration::annotation::InspiredAnnotation;
pub use crate::bot::agent::plugin::inspiration::compact::BotCompact;
pub use crate::bot::agent::plugin::inspiration::state::IInspirationEmbedded;
pub use crate::bot::agent::plugin::inspiration::state::InspiredState;

pub struct InspirationPlugin {
    knowledge_registry: KnowledgeRegistry,
}

impl<M, R, S, A> IAgentPlugin<M, R, S, A> for InspirationPlugin
where
    M: IMessage + Send + Sync + 'static,
    R: IResolver<Message = M> + Send,
    S: IInspirationEmbedded + Send + Sync + 'static,
    A: IInspirationAnnotated + Default + Send + Sync + 'static,
{
    fn interceptors(&mut self) -> Vec<DynInterceptor<S, M, A>> {
        vec![Box::new(InspirationInterceptor::<M, S, A>::new(
            self.knowledge_registry.clone(),
        ))]
    }
}

pub fn inspiration_plugin(memory_dir: std::path::PathBuf) -> anyhow::Result<InspirationPlugin> {
    Ok(InspirationPlugin {
        knowledge_registry: KnowledgeRegistry::load(memory_dir)?,
    })
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
    use crate::bot::agent::plugin::inspiration::annotation::IInspirationAnnotated;
    use crate::bot::agent::plugin::inspiration::annotation::InspiredAnnotation;
    use crate::bot::agent::plugin::inspiration::compact::BotCompact;
    use crate::bot::agent::plugin::inspiration::interceptor::InspirationInterceptor;
    use crate::bot::agent::plugin::inspiration::knowledge::KnowledgeEntry;
    use crate::bot::agent::plugin::inspiration::knowledge::KnowledgeRegistry;
    use crate::bot::agent::plugin::inspiration::state::IInspirationEmbedded;
    use crate::bot::agent::state::BotAgentState;
    use crate::bot::agent::state::BotMessageAnnotation;

    fn user(content: &str) -> ChatCompletionMessageParam {
        MessageRef::User { content }.into()
    }

    fn annotated_user(
        content: &str,
        insp_annotation: InspiredAnnotation,
    ) -> AnnotatedMessage<ChatCompletionMessageParam, BotMessageAnnotation> {
        let mut ann = BotMessageAnnotation::default();
        *ann.inspired_annotation_mut() = insp_annotation;
        AnnotatedMessage::new(user(content), ann)
    }

    fn test_registry() -> KnowledgeRegistry {
        KnowledgeRegistry::from_entries(vec![
            KnowledgeEntry::new(
                "member",
                "lb",
                "LB",
                "LB",
                "LB：核心开发，负责 poprako 全系列工具的开发",
            )
            .unwrap(),
            KnowledgeEntry::new(
                "member",
                "nabai",
                "那白",
                "那白",
                "那白：翻译，热爱学习译法，喜欢用告白台词开玩笑",
            )
            .unwrap(),
        ])
    }

    #[tokio::test]
    async fn before_solve_injects_matched_inspiration_before_latest_user_message() {
        let mut interceptor = InspirationInterceptor::<
            ChatCompletionMessageParam,
            BotAgentState,
            BotMessageAnnotation,
        >::new(test_registry());
        let mut state = BotAgentState::default();
        let mut cx = ContextBuilder::<_, BotMessageAnnotation>::new("test-model")
            .messages(vec![user(
                "[channel_id: 1, channel_name: -, sender_id: 2, sender_nickname: LB, sender_channel_nickname: -, sender_prks_id: -, sent_at: now]\n帮我看看",
            )])
            .build();

        interceptor.before_evaluate(&mut state, &mut cx).await;

        assert_eq!(cx.message_count(), 2);
        assert!(
            state
                .inspired_state_mut()
                .active_knowledge_ids
                .contains("member.lb")
        );
        match cx.annotated_messages()[1].message.message_ref() {
            MessageRef::User { content } => assert_eq!(
                content,
                "[channel_id: 1, channel_name: -, sender_id: 2, sender_nickname: LB, sender_channel_nickname: -, sender_prks_id: -, sent_at: now]\n帮我看看"
            ),
            _ => panic!("最后一条消息应该仍然是用户消息"),
        }
        assert_eq!(
            cx.annotated_messages()[0]
                .annotation
                .inspired_annotation()
                .knowledge_id(),
            Some("member.lb")
        );
    }

    #[tokio::test]
    async fn before_solve_injects_inspiration_loaded_from_memory_files() {
        let memory_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("memory");
        let registry = KnowledgeRegistry::load(memory_dir).unwrap();
        let mut interceptor = InspirationInterceptor::<
            ChatCompletionMessageParam,
            BotAgentState,
            BotMessageAnnotation,
        >::new(registry);
        let mut state = BotAgentState::default();
        let mut cx = ContextBuilder::<_, BotMessageAnnotation>::new("test-model")
            .messages(vec![user(
                "[channel_id: 1, channel_name: -, sender_id: 2, sender_nickname: Dryice, sender_channel_nickname: -, sender_prks_id: -, sent_at: now]\n在吗",
            )])
            .build();

        interceptor.before_evaluate(&mut state, &mut cx).await;

        assert_eq!(cx.message_count(), 2);
        assert!(
            state
                .inspired_state_mut()
                .active_knowledge_ids
                .contains("role-assignment.dryice")
        );
        assert_eq!(
            cx.annotated_messages()[0]
                .annotation
                .inspired_annotation()
                .knowledge_id(),
            Some("role-assignment.dryice")
        );
        match cx.annotated_messages()[0].message.message_ref() {
            MessageRef::User { content } => {
                assert_eq!(content, "[灵光一闪][role-assignment][dryice] 职位：嵌字")
            }
            _ => panic!("注入资料应该作为用户上下文消息写入"),
        }
    }

    #[tokio::test]
    async fn before_solve_does_not_duplicate_active_inspiration() {
        let mut interceptor = InspirationInterceptor::<
            ChatCompletionMessageParam,
            BotAgentState,
            BotMessageAnnotation,
        >::new(test_registry());
        let mut state = BotAgentState::default();
        state
            .inspired_state_mut()
            .active_knowledge_ids
            .insert("member.nabai".to_string());
        let mut cx = ContextBuilder::<_, BotMessageAnnotation>::new("test-model")
            .messages(vec![user(
                "[channel_id: 1, channel_name: -, sender_id: 2, sender_nickname: 小明, sender_channel_nickname: -, sender_prks_id: -, sent_at: now]\n那白在吗",
            )])
            .build();

        interceptor.before_evaluate(&mut state, &mut cx).await;

        assert_eq!(cx.message_count(), 1);
    }

    #[tokio::test]
    async fn before_solve_injects_entries_with_same_pattern() {
        let registry = KnowledgeRegistry::from_entries(vec![
            KnowledgeEntry::new("member", "first", "first", "LB", "first").unwrap(),
            KnowledgeEntry::new("member", "second", "second", "LB", "second").unwrap(),
        ]);
        let mut interceptor = InspirationInterceptor::<
            ChatCompletionMessageParam,
            BotAgentState,
            BotMessageAnnotation,
        >::new(registry);
        let mut state = BotAgentState::default();
        let mut cx = ContextBuilder::<_, BotMessageAnnotation>::new("test-model")
            .messages(vec![user(
                "[channel_id: 1, channel_name: -, sender_id: 2, sender_nickname: LB, sender_channel_nickname: -, sender_prks_id: -, sent_at: now]\n帮我看看",
            )])
            .build();

        interceptor.before_evaluate(&mut state, &mut cx).await;

        assert_eq!(cx.message_count(), 3);
        assert!(
            state
                .inspired_state_mut()
                .active_knowledge_ids
                .contains("member.first")
        );
        assert!(
            state
                .inspired_state_mut()
                .active_knowledge_ids
                .contains("member.second")
        );
    }

    #[tokio::test]
    async fn compact_rebuilds_state_from_retained_annotations() {
        let mut state = BotAgentState::default();
        state
            .inspired_state_mut()
            .active_knowledge_ids
            .insert("member.lb".to_string());
        state
            .inspired_state_mut()
            .active_knowledge_ids
            .insert("member.nabai".to_string());
        let mut cx = ContextBuilder::<_, BotMessageAnnotation>::new("test-model")
            .annotated_messages(vec![
                annotated_user(
                    "[灵光一闪][member][那白] 那白：翻译",
                    InspiredAnnotation::with_knowledge_id("member.nabai"),
                ),
                annotated_user("那白在吗", InspiredAnnotation::default()),
            ])
            .build();
        let mut compact =
            BotCompact::<ChatCompletionMessageParam, BotAgentState, BotMessageAnnotation>::default(
            );

        compact.compact(&mut state, &mut cx).await;

        assert_eq!(state.inspired_state_mut().active_knowledge_ids.len(), 1);
        assert!(
            state
                .inspired_state_mut()
                .active_knowledge_ids
                .contains("member.nabai")
        );
    }
}
