pub mod action;
pub mod context;
pub mod message;
pub mod tool;

pub use context::ContextBuilder;

use crate::ai::resolver::action::Action;
use crate::ai::resolver::context::Context;
use crate::ai::resolver::message::IMessage;
use crate::ai::resolver::result::ResolveResult;

#[async_trait::async_trait]
pub trait IResolver {
    type Message: IMessage + 'static;

    async fn resolve<A>(
        &mut self,
        cx: &Context<Self::Message, A>,
    ) -> ResolveResult<Action<<Self::Message as IMessage>::ToolCall>>
    where
        A: Send + Sync + 'static;
}

pub mod result {
    #[derive(Debug)]
    pub enum ResolveError {
        /// API returned no choices.
        NoChoice { message: String },
        /// API returned an error.
        Api { status: u16, message: String },
        /// HTTP request / network layer failure.
        Network { message: String },
        /// JSON (de)serialization failure.
        JsonSerde { message: String },
        /// Something else went wrong.
        Unknown { message: String },
    }

    impl std::fmt::Display for ResolveError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                ResolveError::NoChoice { message } => {
                    write!(f, "API 未返回有效响应：{message}")
                }
                ResolveError::Api { status, message } => {
                    write!(f, "API 错误 (HTTP {status})：{message}")
                }
                ResolveError::Network { message } => {
                    write!(f, "网络错误：{message}")
                }
                ResolveError::JsonSerde { message } => {
                    write!(f, "数据解析错误：{message}")
                }
                ResolveError::Unknown { message } => {
                    write!(f, "未知错误：{message}")
                }
            }
        }
    }

    pub type ResolveResult<T> = std::result::Result<T, ResolveError>;
}
