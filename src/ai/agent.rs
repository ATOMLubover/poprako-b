use std::collections::HashMap;
use std::collections::HashSet;

use crate::ai::agent::tool::DynTool;
use crate::ai::agent::tool::remote::RemoteProxy;
use crate::ai::agent::tool::result::CallOutput;
use crate::ai::agent::tool::result::CallResult;
use crate::ai::agent::tool::result::ExecutionError;
use crate::ai::resolver::IResolver;
use crate::ai::resolver::action::Reason;
use crate::ai::resolver::context::Context;
use crate::ai::resolver::message::{IMessage, MessageRef};
use crate::ai::resolver::tool::IToolCall;

pub mod compact;
pub mod openai;
pub mod tool;

pub type Compact<M> = fn(&mut Context<M>);

pub struct Agent<M, R>
where
    M: IMessage + 'static,
    R: IResolver<Message = M> + Send,
{
    context: Context<M>,
    local_tools: HashMap<String, DynTool>,
    remote_proxy: Option<RemoteProxy>,

    resolver: R,

    compact: Option<Compact<M>>,
}

impl<M, R> Agent<M, R>
where
    M: IMessage + 'static,
    R: IResolver<Message = M> + Send,
{
    pub fn from_context(cx: Context<M>, resolver: R) -> Self {
        Self {
            context: cx,
            local_tools: HashMap::new(),
            remote_proxy: None,
            resolver,
            compact: None,
        }
    }

    pub fn set_tools(&mut self, tools: Vec<DynTool>) {
        self.local_tools.clear();

        for tool in tools.into_iter() {
            let def = tool.defination();

            self.local_tools.insert(def.name.clone(), tool);
        }

        self.refresh_tools();
    }

    pub fn push_message(&mut self, message: M) {
        self.context.push_message(message);
    }

    pub fn set_messages(&mut self, messages: Vec<M>) {
        self.context.set_messages(messages);
    }

    /// Replace the first message (system prompt) while keeping the rest intact.
    /// If the message list is empty, the new message is pushed as the sole entry.
    pub fn replace_system_message(&mut self, message: M) {
        let mut messages = self.context.take_messages();
        if messages.is_empty() {
            messages.push(message);
        } else {
            messages[0] = message;
        }
        self.context.set_messages(messages);
    }

    pub fn set_compact(&mut self, compact: Compact<M>) {
        self.compact = Some(compact);
    }

    /// Replace all registered tools, returning the old ones.
    pub fn replace_tools(&mut self, tools: Vec<DynTool>) -> Vec<DynTool> {
        let old: Vec<DynTool> = self.local_tools.drain().map(|(_, v)| v).collect();

        for tool in tools {
            let def = tool.defination();
            self.local_tools.insert(def.name.clone(), tool);
        }
        self.refresh_tools();

        old
    }

    /// Run the agent loop. Returns the final assistant text response, or `None`
    /// if the resolver failed before producing a final answer.
    pub async fn solve(&mut self) -> Option<String> {
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
                        let result = self.handle_call(call).await;
                        let message = match &result {
                            Ok(output) => M::from(MessageRef::Tool {
                                tool_call_id: &output.call_id,
                                content: &output.content,
                            }),
                            Err(e) => {
                                let err_msg = format!("{:?}", e);
                                M::from(MessageRef::Tool {
                                    tool_call_id: call.id(),
                                    content: &err_msg,
                                })
                            }
                        };

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

    pub fn compact(&mut self) {
        if let Some(compact) = self.compact {
            compact(&mut self.context);
        }
    }

    async fn handle_call(&mut self, call: &M::ToolCall) -> CallResult {
        if let Some(tool) = self.local_tools.get_mut(call.name()) {
            let content = tool.execute(call.args()).await?;
            return Ok(CallOutput::new(call.id().to_string(), content));
        }

        if let Some(remote_proxy) = &self.remote_proxy
            && remote_proxy.has_tool(call.name())
        {
            return remote_proxy.handle_call(call).await;
        }

        Err(ExecutionError::exec_fail(format!(
            "tool not found: {}",
            call.name()
        )))
    }

    fn refresh_tools(&mut self) {
        let mut defs = Vec::with_capacity(self.local_tools.len());
        let mut local_names = HashSet::with_capacity(self.local_tools.len());

        for tool in self.local_tools.values() {
            let def = tool.defination();
            local_names.insert(def.name.clone());
            defs.push(def);
        }

        if let Some(remote_proxy) = &self.remote_proxy {
            for def in remote_proxy.tool_definations() {
                if local_names.contains(&def.name) {
                    tracing::warn!(tool = %def.name, "remote tool conflicts with local tool, skip");
                    continue;
                }

                defs.push(def);
            }
        }

        self.context.set_tool_defs(defs);
    }
}

pub struct AgentBuilder<M, R>
where
    M: IMessage + 'static,
    R: IResolver<Message = M> + Send,
{
    context: Context<M>,
    resolver: R,
    tools: Vec<DynTool>,
    remote_proxy: Option<RemoteProxy>,
    compact: Option<Compact<M>>,
}

impl<M, R> AgentBuilder<M, R>
where
    M: IMessage + 'static,
    R: IResolver<Message = M> + Send,
{
    pub fn new(context: Context<M>, resolver: R) -> Self {
        Self {
            context,
            resolver,
            tools: Vec::new(),
            remote_proxy: None,
            compact: None,
        }
    }

    pub fn tools(mut self, tools: Vec<DynTool>) -> Self {
        self.tools = tools;
        self
    }

    pub fn compact(mut self, compact: Compact<M>) -> Self {
        self.compact = Some(compact);
        self
    }

    pub fn remote_proxy(mut self, remote_proxy: Option<RemoteProxy>) -> Self {
        self.remote_proxy = remote_proxy;
        self
    }

    pub fn build(self) -> Agent<M, R> {
        let mut agent = Agent::from_context(self.context, self.resolver);

        agent.compact = self.compact;
        agent.remote_proxy = self.remote_proxy;
        agent.set_tools(self.tools);

        agent
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::PathBuf;

    use crate::ai::agent::tool::local::fs::{CreateFileTool, ReadFileTool};
    use crate::ai::resolver::context::ContextBuilder;
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

        let cx = ContextBuilder::new("deepseek-v4-flash")
            .messages(vec![
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
            ])
            .build();

        let mut agent = AgentBuilder::new(cx, resolver)
            .tools(vec![
                Box::new(CreateFileTool::new(output_dir.clone())),
                Box::new(ReadFileTool::new(output_dir)),
            ])
            .build();

        let result = agent.solve().await;
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
