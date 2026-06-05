use crate::ai::resolver::message::IMessage;
use crate::ai::resolver::tool::ToolDefination;

/// Context for the resolver, containing the conversation history and available tools.
/// It **owns** all messages and tools, so Agent can mutate the context by pushing or deleting
/// new messages and tools as needed.
pub struct Context<M>
where
    M: IMessage + 'static,
{
    model: String,
    messages: Vec<M>,
    tool_defs: Vec<ToolDefination>,
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

    pub fn take_messages(&mut self) -> Vec<M> {
        std::mem::take(&mut self.messages)
    }

    pub fn set_messages(&mut self, messages: Vec<M>) {
        self.messages = messages;
    }

    pub fn tool_defs(&self) -> &[ToolDefination] {
        &self.tool_defs
    }

    pub fn set_tool_defs(&mut self, tool_defs: Vec<ToolDefination>) {
        self.tool_defs = tool_defs;
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    pub fn set_model(&mut self, model: String) {
        self.model = model;
    }

    /// Replace the first message (system prompt) while keeping the rest intact.
    /// If the message list is empty, the new message is pushed as the sole entry.
    pub fn set_system_message(&mut self, message: M) {
        if self.messages.is_empty() {
            self.messages.push(message);
        } else {
            self.messages[0] = message;
        }
    }

    /// Return a cloned copy of all messages.
    pub fn snapshot_messages(&self) -> Vec<M>
    where
        M: Clone,
    {
        self.messages.to_vec()
    }
}

pub struct ContextBuilder<M>
where
    M: IMessage + 'static,
{
    model: String,
    messages: Vec<M>,
    tool_defs: Vec<ToolDefination>,
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

    pub fn tool_defs(mut self, tool_defs: Vec<ToolDefination>) -> Self {
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
