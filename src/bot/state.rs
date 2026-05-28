use std::collections::VecDeque;

use crate::bot::{agent::BotAgent, message::InputMessage};

pub struct BotState {
    agent: BotAgent,
    // History used for repeat（复读） feature, with a capacity of 3 to limit memory usage.
    history: VecDeque<InputMessage>,
}

impl BotState {
    pub fn new(agent: BotAgent) -> Self {
        Self {
            agent,
            history: VecDeque::with_capacity(3),
        }
    }

    pub fn agent_mut(&mut self) -> &mut BotAgent {
        &mut self.agent
    }

    pub fn push_history(&mut self, msg: InputMessage) {
        if self.history.len() == 3 {
            self.history.pop_front();
        }

        self.history.push_back(msg);
    }

    pub fn history(&self) -> &VecDeque<InputMessage> {
        &self.history
    }
}
