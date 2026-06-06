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
use crate::bot::agent::plugin::inspiration::knowledge::KnowledgeRegistry;

pub use crate::bot::agent::plugin::inspiration::annotation::IInspirationAnnotated;
pub use crate::bot::agent::plugin::inspiration::annotation::InspiredAnnotation;
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
    fn apply(&self, builder: AgentBuilder<M, R, S, A>) -> AgentBuilder<M, R, S, A> {
        builder
            .compact(InspirationCompact::<M, S, A>::default())
            .interceptor(InspirationInterceptor::<M, S, A>::new(
                self.knowledge_registry.clone(),
            ))
    }
}

pub fn plugin_inspiration(memory_dir: std::path::PathBuf) -> anyhow::Result<InspirationPlugin> {
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
    use crate::bot::agent::plugin::inspiration::compact::InspirationCompact;
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
            KnowledgeEntry {
                id: "member.lb".to_string(),
                pattern: "LB".to_string(),
                content: "LB：核心开发，负责 poprako 全系列工具的开发".to_string(),
            },
            KnowledgeEntry {
                id: "member.nabai".to_string(),
                pattern: "那白".to_string(),
                content: "那白：翻译，热爱学习译法，喜欢用告白台词开玩笑".to_string(),
            },
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

        interceptor.before_solve(&mut state, &mut cx).await;

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

        interceptor.before_solve(&mut state, &mut cx).await;

        assert_eq!(cx.message_count(), 1);
    }

    #[tokio::test]
    async fn before_solve_injects_entries_with_same_pattern() {
        let registry = KnowledgeRegistry::from_entries(vec![
            KnowledgeEntry {
                id: "member.first".to_string(),
                pattern: "LB".to_string(),
                content: "first".to_string(),
            },
            KnowledgeEntry {
                id: "member.second".to_string(),
                pattern: "LB".to_string(),
                content: "second".to_string(),
            },
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

        interceptor.before_solve(&mut state, &mut cx).await;

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
                    "[注入上下文：灵感资料]\n来源：系统\n编号：member.nabai\n说明：这不是真实用户发言。只把它当作当前对话的背景资料，不要直接回应本消息。\n\n那白：翻译\n[/注入上下文]",
                    InspiredAnnotation::with_knowledge_id("member.nabai"),
                ),
                annotated_user("那白在吗", InspiredAnnotation::default()),
            ])
            .build();
        let mut compact = InspirationCompact::<
            ChatCompletionMessageParam,
            BotAgentState,
            BotMessageAnnotation,
        >::default();

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
