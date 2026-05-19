pub mod openai;

pub mod action;
pub mod tool;

use crate::ai::message::Message;
use crate::ai::resolver::action::Action;
use crate::ai::resolver::result::ResolveResult;
use crate::ai::resolver::tool::Tool;

pub struct Context {
    pub model: String,
    pub messages: Vec<Message>,
    pub tools: Vec<Tool>,
}

impl Context {
    pub fn new(model: impl Into<String>, messages: Vec<Message>) -> Self {
        Self {
            model: model.into(),
            messages,
            tools: Vec::new(),
        }
    }
}

#[async_trait::async_trait]
pub trait Resolver: Send {
    async fn resolve(&mut self, cx: &Context) -> ResolveResult<Action>;
}

pub mod result {
    #[derive(Debug)]
    pub enum ResolveError {
        /// API returned no choices.
        NoResponse,
        /// OpenAI API returned an error.
        ApiError { status: u16, message: String },
        /// HTTP request / network layer failure.
        RequestError(String),
        /// JSON (de)serialization failure.
        JsonError(String),
        /// Something else went wrong.
        Other(String),
    }

    pub type ResolveResult<T> = std::result::Result<T, ResolveError>;
}
