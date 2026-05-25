use std::collections::HashMap;

use crate::ai::agent::tool::DynTool;
use crate::ai::agent::tool::result::ToolOutput;
use crate::ai::resolver::IResolver;
use crate::ai::resolver::action::Reason;
use crate::ai::resolver::context::Context;
use crate::ai::resolver::message::{IMessage, MessageRef};
use crate::ai::resolver::tool::IToolCall;

pub mod openai;

pub mod tool;

pub struct Agent<M, R>
where
    M: IMessage + 'static,
    R: IResolver<Message = M> + Send,
{
    context: Context<M>,
    tools: HashMap<String, DynTool>,

    resolver: R,
}

impl<M, R> Agent<M, R>
where
    M: IMessage + 'static,
    R: IResolver<Message = M> + Send,
{
    pub fn from_context(cx: Context<M>, resolver: R) -> Self {
        Self {
            context: cx,
            tools: HashMap::new(),
            resolver,
        }
    }

    pub fn set_tools(&mut self, tools: Vec<DynTool>) {
        let mut tool_defs = vec![];

        for tool in tools.into_iter() {
            let def = tool.defination();

            tool_defs.push(def.clone());
            self.tools.insert(def.name.clone(), tool);
        }

        self.context.set_tools(tool_defs);
    }

    pub fn set_messages(&mut self, messages: Vec<M>) {
        self.context.set_messages(messages);
    }

    /// Replace all registered tools, returning the old ones.
    pub fn replace_tools(&mut self, tools: Vec<DynTool>) -> Vec<DynTool> {
        let old: Vec<DynTool> = self.tools.drain().map(|(_, v)| v).collect();

        let mut defs = Vec::with_capacity(tools.len());
        for tool in tools {
            let def = tool.defination();
            defs.push(def.clone());
            self.tools.insert(def.name.clone(), tool);
        }
        self.context.set_tools(defs);

        old
    }

    /// Run the agent loop. Returns the final assistant text response, or `None`
    /// if the resolver failed before producing a final answer.
    pub async fn run_loop(&mut self) -> Option<String> {
        loop {
            let action = match self.resolver.resolve(&self.context).await {
                Ok(action) => {
                    tracing::info!("resolver produced action: {:?}", action);
                    action
                }
                Err(e) => {
                    tracing::error!("resolve failed: {:?}", e);
                    return None;
                }
            };

            let reason = action.reason.clone();

            // Extract finish content before action is consumed.
            let finish_content = if matches!(reason, Reason::Finish) {
                action.content.clone()
            } else {
                None
            };

            // Process tool calls from the local action — no borrow on self.context.
            let tool_messages: Vec<M> = match &action.tool_calls {
                None => Vec::new(),
                Some(calls) => {
                    let mut messages = Vec::with_capacity(calls.len());
                    for call in calls {
                        let message = self
                            .dispatch_tool_call(call)
                            .await
                            .map(|output| {
                                M::from(MessageRef::Tool {
                                    tool_call_id: &output.id,
                                    content: &output.content,
                                })
                            })
                            .unwrap_or_else(|e| {
                                M::from(MessageRef::Tool {
                                    tool_call_id: call.id(),
                                    content: &e,
                                })
                            });

                        messages.push(message);
                    }

                    messages
                }
            };

            // Push assistant message and tool results to context together.
            self.context.push_message(action.into());
            for msg in tool_messages {
                self.context.push_message(msg);
            }

            match reason {
                Reason::Finish => return finish_content,
                // FIXME: ToolCall, Length, Unknown → continue the loop.
                _ => continue,
            }
        }
    }

    async fn dispatch_tool_call(&mut self, call: &M::ToolCall) -> Result<ToolOutput, String> {
        let tool = match self.tools.get_mut(call.name()) {
            Some(tool) => tool,
            None => return Err(format!("tool not found: {}", call.name())),
        };

        tool.execute(call.args())
            .await
            .map(|content| ToolOutput::new(call.id(), content))
            .map_err(|e| format!("{:?}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::PathBuf;

    use crate::ai::agent::tool::local::{CreateFileTool, ReadFileTool};
    use crate::ai::resolver::openai::OpenAiResolver;

    use openai_oxide::types::chat::ChatCompletionMessageParam;
    use openai_oxide::types::chat::UserContent;

    #[tokio::test]
    async fn create_file_and_read() {
        dotenvy::dotenv().ok();
        let _ = tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .try_init();

        let output_dir = PathBuf::from("tests/output");
        let target_file = output_dir.join("hello.txt");

        // Create the output directory; leftover files from previous runs are
        // cleaned up by CreateFileTool's Drop when the agent goes out of scope.
        std::fs::create_dir_all(&output_dir).expect("should create tests/output");

        let resolver = OpenAiResolver::from_env();

        let mut cx = Context::new("deepseek-v4-flash".to_string());
        cx.set_messages(vec![
            ChatCompletionMessageParam::System {
                content:
                    "You are a helpful assistant. Use the create_file tool to create files and \
                          the read_file tool to read them. The path parameter is relative to the \
                          base directory. When asked to create a file, use the tool directly - \
                          do not ask for confirmation."
                        .to_string(),
                name: None,
            },
            ChatCompletionMessageParam::User {
                content: UserContent::Text(
                    "Create a file at 'hello.txt' with the content 'hello from agent', \
                     then read it back to confirm the content."
                        .to_string(),
                ),
                name: None,
            },
        ]);

        let mut agent = Agent::from_context(cx, resolver);
        agent.set_tools(vec![
            Box::new(CreateFileTool::new(output_dir.clone())),
            Box::new(ReadFileTool::new(output_dir)),
        ]);

        let result = agent.run_loop().await;
        assert!(result.is_some(), "agent should return a final response");
        assert!(
            target_file.exists(),
            "expected file at {}",
            target_file.display()
        );

        let contents = std::fs::read_to_string(&target_file).expect("should read created file");
        assert!(
            contents.contains("hello from agent"),
            "file should contain 'hello from agent', got: {contents}"
        );

        // Pop CreateFileTool out before agent drops, then clean up the file directly.
        let _old_tools = agent.replace_tools(vec![]);
        let _ = std::fs::remove_file(&target_file);
    }
}
