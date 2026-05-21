use crate::ai::resolver::message::IMessage;
use crate::ai::resolver::tool::Tool;

/// Context for the resolver, containing the conversation history and available tools.
/// It **owns** all messages and tools, so Agent can mutate the context by pushing or deleting
/// new messages and tools as needed.
pub struct Context<M>
where
    M: IMessage + 'static,
{
    model: String,
    messages: Vec<M>,
    tools: Vec<Tool>,
}

impl<M> Context<M>
where
    M: IMessage + 'static,
{
    pub fn new(model: String) -> Self {
        Self {
            model,
            messages: Vec::new(),
            tools: Vec::new(),
        }
    }

    pub fn messages(&self) -> &[M] {
        &self.messages
    }

    pub fn push_message(&mut self, message: M) {
        self.messages.push(message);
    }

    pub fn set_messages(&mut self, messages: Vec<M>) {
        self.messages = messages;
    }

    pub fn with_messages(mut self, messages: Vec<M>) -> Self {
        self.messages = messages;
        self
    }

    pub fn tools(&self) -> &[Tool] {
        &self.tools
    }

    pub fn set_tools(&mut self, tools: Vec<Tool>) {
        self.tools = tools;
    }

    pub fn with_tools(mut self, tools: Vec<Tool>) -> Self {
        self.tools = tools;
        self
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    pub fn set_model(&mut self, model: String) {
        self.model = model;
    }

    pub fn with_model(mut self, model: String) -> Self {
        self.model = model;
        self
    }
}
