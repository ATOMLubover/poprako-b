pub mod openai;

pub mod action;
pub mod context;
pub mod message;
pub mod tool;

use crate::ai::resolver::action::Action;
use crate::ai::resolver::context::Context;
use crate::ai::resolver::message::IMessage;
use crate::ai::resolver::result::ResolveResult;

#[async_trait::async_trait]
pub trait Resolver: Send {
    type Message: IMessage + 'static;

    async fn resolve(&mut self, cx: &Context<Self::Message>) -> ResolveResult<Action>;
}

pub mod result {
    #[derive(Debug)]
    pub enum ResolveError {
        /// API returned no choices.
        InvalidResponse,
        /// OpenAI API returned an error.
        ApiError { status: u16, message: String },
        /// HTTP request / network layer failure.
        RequestError { message: String },
        /// JSON (de)serialization failure.
        JsonError { message: String },
        /// Something else went wrong.
        Unknown { message: String },
    }

    pub type ResolveResult<T> = std::result::Result<T, ResolveError>;
}
