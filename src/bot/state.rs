use std::collections::VecDeque;

use crate::bot::agent::BotAgent;
use crate::bot::message::InputMessage;

pub struct BotState {
    agent: BotAgent,

    self_qid: i64,
    developer_qid: Option<i64>,

    // History used for repeat（复读） feature, with a capacity of 3 to limit memory usage.
    history: VecDeque<InputMessage>,
    // The last text the bot repeated, to avoid repeating the same sentence over and over.
    last_repeat: Option<String>,
}

impl BotState {
    pub fn new(agent: BotAgent, self_qid: i64) -> Self {
        let developer_qid = std::env::var("DEVELOPER")
            .ok()
            .and_then(|v| v.parse::<i64>().ok());

        Self {
            agent,
            self_qid,
            history: VecDeque::with_capacity(3),
            last_repeat: None,
            developer_qid,
        }
    }

    pub fn agent_mut(&mut self) -> &mut BotAgent {
        &mut self.agent
    }

    pub fn self_qid(&self) -> i64 {
        self.self_qid
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

    pub fn last_repeat(&self) -> Option<&str> {
        self.last_repeat.as_deref()
    }

    pub fn set_last_repeat(&mut self, text: String) {
        self.last_repeat = Some(text);
    }

    pub fn is_developer(&self, user_qid: i64) -> bool {
        self.developer_qid == Some(user_qid)
    }

    pub fn is_self(&self, user_qid: i64) -> bool {
        self.self_qid == user_qid
    }
}
