use crate::ai::resolver::message::Message;
use crate::ai::resolver::tool::Tool;

pub struct Context {
    model: String,
    messages: Vec<Message>,
    tools: Vec<Tool>,
}

impl Context {
    pub fn new(model: String) -> Self {
        Self {
            model,
            messages: Vec::new(),
            tools: Vec::new(),
        }
    }

    pub fn with_messages(mut self, messages: Vec<Message>) -> Self {
        self.messages = messages;
        self
    }

    pub fn with_tools(mut self, tools: Vec<Tool>) -> Self {
        self.tools = tools;
        self
    }

    pub fn push_message(&mut self, message: Message) {
        self.messages.push(message);
    }

    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    pub fn tools(&self) -> &[Tool] {
        &self.tools
    }
}
