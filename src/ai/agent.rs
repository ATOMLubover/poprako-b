use crate::ai::agent::tools::dispatch_tool_call;
use crate::ai::resolver::Resolver;
use crate::ai::resolver::action::Reason;
use crate::ai::resolver::context::Context;
use crate::ai::resolver::message::Message;
use crate::ai::resolver::tool::Tool;

pub mod prompts;
pub mod tools;

pub struct Agent<R>
where
    R: Resolver + Send,
{
    context: Context,
    resolver: R,
}

impl<R> Agent<R>
where
    R: Resolver + Send,
{
    pub fn from_context(cx: Context, resolver: R) -> Self {
        Self {
            context: cx,
            resolver,
        }
    }

    pub fn with_tools(mut self, tools: Vec<Tool>) -> Self {
        self.context = self.context.with_tools(tools);
        self
    }

    /// Run the agent loop. Returns the final assistant text response, or `None`
    /// if the resolver failed before producing a final answer.
    pub async fn run_loop(&mut self) -> Option<String> {
        loop {
            let action = match self.resolver.resolve(&self.context).await {
                Ok(action) => action,
                Err(e) => {
                    tracing::error!("resolve failed: {:?}", e);
                    return None;
                }
            };

            // Extract fields before `action` is moved into a Message.
            let reason = action.reason.clone();
            let content = action.content.clone();
            let tool_calls = action.tool_calls.clone();

            // Push the assistant message (with tool_calls if any).
            self.context.push_message(action.into());

            // Dispatch tool calls and feed results back as Tool messages.
            if let Some(calls) = tool_calls {
                for call in &calls {
                    let tool_msg = match dispatch_tool_call(call).await {
                        Ok(output) => Message::Tool {
                            tool_call_id: output.id,
                            content: output.content,
                        },
                        Err(e) => Message::Tool {
                            tool_call_id: call.id.clone(),
                            content: format!("Error: {:?}", e),
                        },
                    };
                    self.context.push_message(tool_msg);
                }
            }

            match reason {
                Reason::Finish => return content,
                // ToolCall, Length, Unknown → continue the loop.
                _ => continue,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::PathBuf;

    use crate::ai::agent::tools::local::command_line_tool;
    use crate::ai::resolver::message::Message;
    use crate::ai::resolver::openai::OpenAiResolver;

    /// Drop guard that removes created test artefacts.
    struct Cleanup {
        paths: Vec<PathBuf>,
    }

    impl Cleanup {
        fn new(paths: Vec<PathBuf>) -> Self {
            Self { paths }
        }
    }

    impl Drop for Cleanup {
        fn drop(&mut self) {
            for path in &self.paths {
                if path.is_file() {
                    let _ = std::fs::remove_file(path);
                }
                if path.is_dir() {
                    let _ = std::fs::remove_dir(path);
                }
            }
        }
    }

    #[tokio::test]
    async fn create_file_in_tests_output() {
        dotenvy::dotenv().ok();

        let output_dir = PathBuf::from("tests/output");
        let target_file = output_dir.join("hello.txt");

        // Ensure clean starting state and register final cleanup.
        let _guard = Cleanup::new(vec![target_file.clone(), output_dir.clone()]);

        // Pre-cleanup in case a previous run left artefacts.
        let _ = std::fs::remove_file(&target_file);
        let _ = std::fs::remove_dir(&output_dir);
        std::fs::create_dir_all(&output_dir).expect("should create tests/output");

        let resolver = OpenAiResolver::from_env();
        let cx = Context::new("deepseek-v4-flash".to_string()).with_messages(vec![
            Message::System {
                name: None,
                content: "You are a helpful assistant. Use the command_line tool to execute shell \
                     commands. When asked to create a file, use the tool directly — do not ask \
                     for confirmation."
                    .to_string(),
            },
            Message::User {
                name: None,
                content: format!(
                    "Create a file at {} with the content 'hello from agent'",
                    target_file.display()
                ),
            },
        ]);

        let mut agent = Agent::from_context(cx, resolver).with_tools(vec![command_line_tool()]);

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
    }
}
