use serde::Deserialize;
use tokio::sync::mpsc;

use crate::ai::agent_impl::deepseek::DeepSeekAgentBuilder;
use crate::ai::resolver::context::Context;
use crate::ai::resolver::message::MessageOwned;
use crate::ai::resolver_impl::deepseek::DeepSeekResolver;
use crate::ai::resolver_impl::deepseek::data_object::DeepSeekMessage;
use crate::bot::event::ReviewFollowupEvent;

pub fn reviewer_system_prompt(respond_id: &str) -> String {
    format!(
        "# 白杨子潜意识\n\
         \n\
         ## 身份\n\
         你是白杨子思考过程中的内在觉察层。你之上还有一个对外发言的主人格，\
         你们共享同一段对话上下文。你负责审视主人格的回答是否完备，不直接与用户对话。\n\
         \n\
         ## 审视范围\n\
         收到审视指令后，检查 respond_id 为 {} 的那轮对话中的用户输入与助手回答，\
         结合完整对话上下文判断回答是否充分。\n\
         \n\
         ## 审视维度\n\
         1. 明确遗漏 — 用户明确提出的问题或要求，回答未覆盖\n\
         2. 事实性错误 — 回答中的事实描述与上下文或常识明显矛盾\n\
         3. 逻辑错误 — 推理链条或因果关系存在断裂\n\
         4. 未执行要求 — 用户给出的明确动作指令未被落实\n\
         \n\
         ## 判断准则\n\
         - 回答在以上四个维度均无问题，输出 needs_followup=false\n\
         - 需要补充时，feedback 只写需补充或修正的具体内容，不要复述已有正确部分\n\
         \n\
         ## 输出格式\n\
         只输出严格 JSON，不要 markdown 代码块。\n\
         \n\
         JSON schema:\n\
         {{\"needs_followup\":bool,\"respond_id\":\"{}\",\"feedback\":\"string\",\"target_summary\":\"string\"}}",
        respond_id, respond_id
    )
}

#[derive(Debug, Deserialize)]
pub struct ReviewDecision {
    pub needs_followup: bool,
    pub respond_id: String,
    pub feedback: String,
    pub target_summary: String,
}

pub fn parse_review_decision(content: &str) -> Option<ReviewDecision> {
    serde_json::from_str(content.trim()).ok()
}

pub async fn run_reviewer(
    mut cx: Context<DeepSeekMessage, impl Default + Send + Sync + 'static>,
    channel_id: String,
    respond_id: String,
    event_send: mpsc::Sender<ReviewFollowupEvent>,
) {
    cx.set_tool_defs(Vec::new());

    let system_prompt = reviewer_system_prompt(&respond_id);
    cx.set_system_message(MessageOwned::System { content: system_prompt }.into());

    let resolver = DeepSeekResolver::from_env();
    let mut agent = DeepSeekAgentBuilder::<(), _>::new(cx, resolver).build();
    let user_message = MessageOwned::User { content: "审视上一轮回答".into() }.into();

    let Some(output) = agent.evaluate(user_message).await else {
        return;
    };

    let Some(decision) = parse_review_decision(&output) else {
        tracing::warn!(
            respond_id = respond_id.as_str(),
            "reviewer returned invalid JSON"
        );
        return;
    };

    if !decision.needs_followup || decision.respond_id != respond_id {
        return;
    }

    let event = ReviewFollowupEvent {
        channel_id,
        respond_id: decision.respond_id,
        feedback: decision.feedback,
        target_summary: decision.target_summary,
    };

    if event_send.send(event).await.is_err() {
        tracing::warn!("review feedback event bus dropped");
    }
}
