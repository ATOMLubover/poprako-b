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
    tool_defs: Vec<ToolDef>,
}

impl<M> Context<M>
where
    M: IMessage + 'static,
{
    pub fn new(model: String) -> Self {
        Self {
            model,
            messages: Vec::new(),
            tool_defs: Vec::new(),
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

    pub fn tool_defs(&self) -> &[ToolDef] {
        &self.tool_defs
    }

    pub fn set_tool_defs(&mut self, tool_defs: Vec<ToolDef>) {
        self.tool_defs = tool_defs;
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    pub fn set_model(&mut self, model: String) {
        self.model = model;
    }
}

pub struct ContextBuilder<M>
where
    M: IMessage + 'static,
{
    model: String,
    messages: Vec<M>,
    tool_defs: Vec<ToolDef>,
}

impl<M> ContextBuilder<M>
where
    M: IMessage + 'static,
{
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            messages: Vec::new(),
            tool_defs: Vec::new(),
        }
    }

    pub fn messages(mut self, messages: Vec<M>) -> Self {
        self.messages = messages;
        self
    }

    pub fn tool_defs(mut self, tool_defs: Vec<ToolDef>) -> Self {
        self.tool_defs = tool_defs;
        self
    }

    pub fn build(self) -> Context<M> {
        Context {
            model: self.model,
            messages: self.messages,
            tool_defs: self.tool_defs,
        }
    }
}
