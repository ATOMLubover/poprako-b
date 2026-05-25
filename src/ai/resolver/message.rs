use crate::ai::resolver::action::Action;
use crate::ai::resolver::tool::IToolCall;

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

pub trait IMessage:
    From<Action<Self::ToolCall>> + for<'a> From<MessageRef<'a, Self::ToolCall>>
{
    // ToolCall will be specified by the implementor, as we do not have to
    // make a generic constraint on the IMessage trait itself.
    type ToolCall: IToolCall + std::fmt::Debug;

    fn message_ref(&self) -> MessageRef<'_, Self::ToolCall>;

    fn is_user(&self) -> bool {
        matches!(self.message_ref(), MessageRef::User { .. })
    }
}
