use serde::Serialize;

use crate::ai::resolver::action::Action;
use crate::ai::resolver::tool::IToolCall;

#[derive(Serialize)]
pub struct PluginSystemItem {}

#[derive(Serialize)]
pub struct EmbeddedSystemItem {}

fn formatted_system_string(
    embedded: Vec<EmbeddedSystemItem>,
    plugins: Vec<PluginSystemItem>,
) -> String {
    todo!()
}

#[derive(Debug)]
pub enum MessageRef<'a, C>
where
    C: IToolCall,
{
    System {
        content: &'a str,
    },
    User {
        content: &'a str,
    },
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
    System {
        content: String,
    },
    User {
        content: String,
    },
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

impl<C> MessageOwned<C>
where
    C: IToolCall,
{
    pub fn formatted_system(
        embedded: Vec<EmbeddedSystemItem>,
        plugins: Vec<PluginSystemItem>,
    ) -> Self {
        MessageOwned::System {
            content: formatted_system_string(embedded, plugins),
        }
    }
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
    // ToolCall will be specified by the implementor, as we do not have to
    // make a generic constraint on the IMessage trait itself.
    type ToolCall: IToolCall + std::fmt::Debug;

    /// Returns a reference to the message content, which can be used for processing.
    /// Reduces unnecessary clones.
    fn message_ref(&self) -> MessageRef<'_, Self::ToolCall>;

    /// Returns the role of the message, which can be System, User, Assist, or Tool.
    fn role(&self) -> MessageRole {
        match self.message_ref() {
            MessageRef::System { .. } => MessageRole::System,
            MessageRef::User { .. } => MessageRole::User,
            MessageRef::Assist { .. } => MessageRole::Assist,
            MessageRef::Tool { .. } => MessageRole::Tool,
        }
    }
}
