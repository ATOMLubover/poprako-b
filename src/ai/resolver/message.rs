use crate::ai::resolver::action::Action;
use crate::ai::resolver::tool::IToolCall;

// ---------------------------------------------------------------------------
// Message types
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum MessageRef<'a, C>
where
    C: IToolCall,
{
    System { content: &'a str },
    User { content: &'a str },
    Assist {
        content: Option<&'a str>,
        tool_calls: Option<&'a [C]>,
        refusal: Option<&'a str>,
    },
    Tool {
        tool_call_id: &'a str,
        content: &'a str,
    },
}

pub enum MessageOwned<C>
where
    C: IToolCall,
{
    System { content: String },
    User { content: String },
    Assist {
        content: Option<String>,
        tool_calls: Option<Vec<C>>,
        refusal: Option<String>,
    },
    Tool {
        tool_call_id: String,
        content: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageRole {
    System,
    User,
    Assist,
    Tool,
}

pub trait IMessage:
    From<Action<Self::ToolCall>>
    + for<'a> From<MessageRef<'a, Self::ToolCall>>
    + From<MessageOwned<Self::ToolCall>>
{
    type ToolCall: IToolCall + std::fmt::Debug;

    fn message_ref(&self) -> MessageRef<'_, Self::ToolCall>;

    fn role(&self) -> MessageRole {
        match self.message_ref() {
            MessageRef::System { .. } => MessageRole::System,
            MessageRef::User { .. } => MessageRole::User,
            MessageRef::Assist { .. } => MessageRole::Assist,
            MessageRef::Tool { .. } => MessageRole::Tool,
        }
    }
}

