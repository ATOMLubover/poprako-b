use std::collections::VecDeque;

use crate::bot::agent::BotAgent;

pub struct BotState {
    agent: BotAgent,

    self_id: String,
    developer_id: Option<String>,

    // History used for repeat（复读） feature, with a capacity of 3 to limit memory usage.
    history: VecDeque<String>,
    // The last text the bot repeated, to avoid repeating the same sentence over and over.
    last_repeat: Option<String>,
}

impl BotState {
    pub fn new(agent: BotAgent, self_id: impl Into<String>) -> Self {
        let developer_id = std::env::var("DEVELOPER").ok();

        Self {
            agent,
            self_id: self_id.into(),
            history: VecDeque::with_capacity(3),
            last_repeat: None,
            developer_id,
        }
    }

    pub fn agent_mut(&mut self) -> &mut BotAgent {
        &mut self.agent
    }

    pub fn self_id(&self) -> &str {
        &self.self_id
    }

    pub fn push_history_text(&mut self, text: String) {
        if self.history.len() == 3 {
            self.history.pop_front();
        }

        self.history.push_back(text);
    }

    pub fn history(&self) -> &VecDeque<String> {
        &self.history
    }

    pub fn last_repeat(&self) -> Option<&str> {
        self.last_repeat.as_deref()
    }

    pub fn set_last_repeat(&mut self, text: String) {
        self.last_repeat = Some(text);
    }

    pub fn is_developer(&self, user_id: &str) -> bool {
        self.developer_id.as_deref() == Some(user_id)
    }

    pub fn is_self(&self, user_id: &str) -> bool {
        self.self_id == user_id
    }
}
