use crate::ai::resolver::message::IMessage;
use crate::ai::resolver::tool::ToolDefination;

#[derive(Debug, Clone)]
pub struct AnnotatedMessage<M, A = ()>
where
    M: IMessage + 'static,
{
    pub message: M,
    pub annotation: A,
}

impl<M, A> AnnotatedMessage<M, A>
where
    M: IMessage + 'static,
{
    pub fn new(message: M, annotation: A) -> Self {
        Self {
            message,
            annotation,
        }
    }
}

/// Context for the resolver, containing the conversation history and available tools.
/// It **owns** all messages and tools, so Agent can mutate the context by pushing or deleting
/// new messages and tools as needed.
pub struct Context<M, A = ()>
where
    M: IMessage + 'static,
{
    model: String,
    messages: Vec<AnnotatedMessage<M, A>>,
    tool_defs: Vec<ToolDefination>,
}

impl<M, A> Context<M, A>
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

    pub fn annotated_messages(&self) -> &[AnnotatedMessage<M, A>] {
        self.messages.as_slice()
    }

    pub fn message_at(&self, index: usize) -> Option<&M> {
        self.messages.get(index).map(|message| &message.message)
    }

    pub fn messages(&self) -> impl Iterator<Item = &M> {
        self.messages.iter().map(|message| &message.message)
    }

    pub fn push_annotated_message(&mut self, message: AnnotatedMessage<M, A>) {
        self.messages.push(message);
    }

    pub fn take_annotated_messages(&mut self) -> Vec<AnnotatedMessage<M, A>> {
        std::mem::take(&mut self.messages)
    }

    pub fn set_annotated_messages(&mut self, messages: Vec<AnnotatedMessage<M, A>>) {
        self.messages = messages;
    }

    pub fn message_count(&self) -> usize {
        self.messages.len()
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
}

impl<M, A> Context<M, A>
where
    M: IMessage + 'static,
    A: Default,
{
    pub fn push_message(&mut self, message: M) {
        self.messages
            .push(AnnotatedMessage::new(message, A::default()));
    }

    pub fn take_messages(&mut self) -> Vec<M> {
        std::mem::take(&mut self.messages)
            .into_iter()
            .map(|message| message.message)
            .collect()
    }

    pub fn set_messages(&mut self, messages: Vec<M>) {
        self.messages = messages
            .into_iter()
            .map(|message| AnnotatedMessage::new(message, A::default()))
            .collect();
    }

    /// Replace the first message (system prompt) while keeping the rest intact.
    /// If the message list is empty, the new message is pushed as the sole entry.
    pub fn set_system_message(&mut self, message: M) {
        if self.messages.is_empty() {
            self.push_message(message);
        } else {
            self.messages[0].message = message;
        }
    }

    /// Return a cloned copy of all messages.
    pub fn snapshot_messages(&self) -> Vec<M>
    where
        M: Clone,
    {
        self.messages().cloned().collect()
    }
}

pub struct ContextBuilder<M, A = ()>
where
    M: IMessage + 'static,
{
    model: String,
    messages: Vec<AnnotatedMessage<M, A>>,
    tool_defs: Vec<ToolDefination>,
}

impl<M, A> ContextBuilder<M, A>
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

    pub fn annotated_messages(mut self, messages: Vec<AnnotatedMessage<M, A>>) -> Self {
        self.messages = messages;
        self
    }

    pub fn tool_defs(mut self, tool_defs: Vec<ToolDefination>) -> Self {
        self.tool_defs = tool_defs;
        self
    }

    pub fn build(self) -> Context<M, A> {
        Context {
            model: self.model,
            messages: self.messages,
            tool_defs: self.tool_defs,
        }
    }
}

impl<M, A> ContextBuilder<M, A>
where
    M: IMessage + 'static,
    A: Default,
{
    pub fn messages(mut self, messages: Vec<M>) -> Self {
        self.messages = messages
            .into_iter()
            .map(|message| AnnotatedMessage::new(message, A::default()))
            .collect();
        self
    }
}
