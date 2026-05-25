use crate::bot::agent::BotAgent;

pub struct BotState {
    pub agent: BotAgent,
}

impl BotState {
    pub fn new(agent: BotAgent) -> Self {
        Self { agent }
    }
}
