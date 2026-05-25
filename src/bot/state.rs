use crate::bot::agent::BotAgent;

pub struct BotState {
    agent: BotAgent,
}

impl BotState {
    pub fn new(agent: BotAgent) -> Self {
        Self { agent }
    }

    pub fn agent_mut(&mut self) -> &mut BotAgent {
        &mut self.agent
    }
}
