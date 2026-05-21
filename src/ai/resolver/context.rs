use crate::ai::resolver::message::IMessage;
use crate::ai::resolver::tool::ToolDef;

/// Context for the resolver, containing the conversation history and available tools.
/// It **owns** all messages and tools, so Agent can mutate the context by pushing or deleting
/// new messages and tools as needed.
pub struct Context<M>
where
    M: IMessage + 'static,
{
    model: String,
    messages: Vec<M>,
    tools: Vec<ToolDef>,
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

    pub fn tools(&self) -> &[ToolDef] {
        &self.tools
    }

    pub fn set_tools(&mut self, tools: Vec<ToolDef>) {
        self.tools = tools;
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    pub fn set_model(&mut self, model: String) {
        self.model = model;
    }
}
