use crate::ai::agent::openai::OpenAiAgent;

/// BotAgent is used for responding to group messages.
pub struct BotAgent {
    agent: OpenAiAgent,
}
