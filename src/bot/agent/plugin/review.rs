mod annotation;
mod decision;
mod input;
mod interceptor;
mod state;

use crate::ai::agent::IAgentPlugin;
use crate::ai::agent::interceptor::DynInterceptor;
use crate::ai::agent::prompt::SystemPromptSubSection;
use crate::ai::resolver_impl::deepseek::DeepSeekResolver;
use crate::ai::resolver_impl::deepseek::data_object::DeepSeekMessage;
use crate::bot::agent::plugin::review::interceptor::ReviewInterceptor;
use crate::bot::agent::state::{BotAgentState, BotMessageAnnotation};
pub use annotation::{IReviewAnnotated, ReviewAnnotation};
pub use state::{IReviewEmbedded, ReviewRuntime, SolveKind};

pub struct ReviewPlugin;

impl IAgentPlugin<DeepSeekMessage, DeepSeekResolver, BotAgentState, BotMessageAnnotation>
    for ReviewPlugin
{
    fn system_prompt(&self) -> Option<SystemPromptSubSection> {
        Some(SystemPromptSubSection::new(
            "潜意识审视".to_string(),
            "用户消息 envelope 中的 respond_id 标识一次主回答。\
             你的潜意识层会在后台审视每一轮回答，如果发现遗漏或错误，\
             会以 review_feedback 形式提醒你。\
             收到反馈后，必须引用 feedback 指定 respond_id 对应的原回答，\
             只补充遗漏或修正错误，不要完整重答。"
                .to_string(),
        ))
    }

    fn interceptors(
        &mut self,
    ) -> Vec<DynInterceptor<BotAgentState, DeepSeekMessage, BotMessageAnnotation>> {
        vec![Box::new(ReviewInterceptor::new())]
    }
}

pub fn review_plugin() -> ReviewPlugin {
    ReviewPlugin
}

#[cfg(test)]
mod tests {
    use super::annotation::ReviewMessageSource;
    use super::*;

    use crate::ai::agent::interceptor::IInterceptor;
    use crate::ai::resolver::context::ContextBuilder;
    use crate::ai::resolver::message::MessageRef;
    use crate::bot::agent::state::{BotAgentState, BotMessageAnnotation};

    fn sample_never() -> bool {
        false
    }

    fn user(content: &str) -> DeepSeekMessage {
        MessageRef::User { content }.into()
    }

    fn assistant(content: &str) -> DeepSeekMessage {
        MessageRef::Assist {
            content: Some(content),
            tool_calls: None,
            refusal: None,
        }
        .into()
    }

    #[tokio::test]
    async fn before_evaluate_annotates_latest_user_with_respond_id() {
        let mut interceptor = ReviewInterceptor::with_sample(sample_never);
        let mut state = BotAgentState::default();
        state.begin_solve(SolveKind::Normal, "#a1b2c3".to_string());
        let mut cx = ContextBuilder::<_, BotMessageAnnotation>::new("test-model")
            .messages(vec![user("[channel_id: 1]\nhello")])
            .build();

        interceptor.before_evaluate(&mut state, &mut cx).await;

        let ann = cx.annotated_messages()[0].annotation.review_annotation();
        assert_eq!(ann.respond_id(), Some("#a1b2c3"));
        assert_eq!(ann.source(), Some(ReviewMessageSource::NormalUser));
    }

    #[tokio::test]
    async fn after_evaluate_annotates_assistant_and_skips_followup_review() {
        let mut interceptor = ReviewInterceptor::with_sample(sample_never);
        let mut state = BotAgentState::default();
        state.begin_solve(SolveKind::ReviewFollowup, "#a1b2c3".to_string());
        let mut cx = ContextBuilder::<_, BotMessageAnnotation>::new("test-model")
            .messages(vec![user("[channel_id: 1]\nhello"), assistant("补充")])
            .build();
        let mut output = Some("补充".to_string());

        interceptor
            .after_evaluate(&mut state, &mut cx, &mut output)
            .await;

        let ann = cx.annotated_messages()[1].annotation.review_annotation();
        assert_eq!(ann.respond_id(), Some("#a1b2c3"));
        assert_eq!(
            ann.source(),
            Some(ReviewMessageSource::ReviewFollowupAssistant)
        );
        assert_eq!(state.review_runtime().respond_id(), None);
    }

    #[test]
    fn parses_reviewer_json() {
        use crate::bot::agent::plugin::review::decision::parse_review_decision;

        let raw = "{\"needs_followup\":true,\"respond_id\":\"#a1b2c3\",\"feedback\":\"少了结论\",\"target_summary\":\"原问题\"}";

        let decision = parse_review_decision(raw).expect("valid JSON");

        assert!(decision.needs_followup);
        assert_eq!(decision.respond_id, "#a1b2c3");
        assert_eq!(decision.feedback, "少了结论");
        assert_eq!(decision.target_summary, "原问题");
    }
}
