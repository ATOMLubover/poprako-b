use std::collections::VecDeque;

use crate::bot::agent::BotAgent;

pub struct BotIdentity {
    self_id: String,
    developer_id: Option<String>,
}

impl BotIdentity {
    fn from_env(self_id: impl Into<String>) -> Self {
        Self {
            self_id: self_id.into(),
            developer_id: std::env::var("DEVELOPER").ok(),
        }
    }

    pub fn self_id(&self) -> &str {
        &self.self_id
    }

    pub fn is_developer(&self, user_id: &str) -> bool {
        self.developer_id.as_deref() == Some(user_id)
    }

    pub fn is_self(&self, user_id: &str) -> bool {
        self.self_id == user_id
    }
}

pub struct RepeatState {
    history: VecDeque<String>,
    last_repeat: Option<String>,
}

impl RepeatState {
    pub fn new() -> Self {
        Self {
            history: VecDeque::with_capacity(3),
            last_repeat: None,
        }
    }

    pub fn push_text(&mut self, text: String) {
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
}

pub struct BotState {
    agent: BotAgent,
    identity: BotIdentity,
    repeat: RepeatState,
}

impl BotState {
    pub fn new(agent: BotAgent, self_id: impl Into<String>) -> Self {
        Self {
            agent,
            identity: BotIdentity::from_env(self_id),
            repeat: RepeatState::new(),
        }
    }

    pub fn agent_mut(&mut self) -> &mut BotAgent {
        &mut self.agent
    }

    pub fn self_id(&self) -> &str {
        self.identity.self_id()
    }

    pub fn repeat_mut(&mut self) -> &mut RepeatState {
        &mut self.repeat
    }

    pub fn push_history_text(&mut self, text: String) {
        self.repeat.push_text(text);
    }

    pub fn is_developer(&self, user_id: &str) -> bool {
        self.identity.is_developer(user_id)
    }

    pub fn is_self(&self, user_id: &str) -> bool {
        self.identity.is_self(user_id)
    }
}
