use std::collections::HashMap;
use std::path::PathBuf;

use openai_oxide::types::chat::{ChatCompletionMessageParam, UserContent};

use crate::ai::agent::plugin::IAgentPlugin;
use crate::ai::agent::tool::DynTool;
use crate::ai::agent::tool::ITool;
use crate::ai::agent::tool::embedded_local::fs::{ListFilesTool, ReadFileTool};
use crate::ai::agent::tool::result::{ExecutionError, ExecutionResult};
use crate::ai::agent_impl::openai::OpenAiAgentBuilder;
use crate::ai::resolver::IResolver;
use crate::ai::resolver::context::ContextBuilder;
use crate::ai::resolver::message::IMessage;
use crate::ai::resolver::tool::{ParamDef, PropDef, ToolDefination};
use crate::ai::resolver_impl::openai::OpenAiResolver;

pub struct RunSubagentsTool {
    default_model: String,
    max_tasks: usize,
    tools_base_dir: PathBuf,
}

impl RunSubagentsTool {
    pub fn new(default_model: String, max_tasks: usize, tools_base_dir: PathBuf) -> Self {
        Self {
            default_model,
            max_tasks,
            tools_base_dir,
        }
    }

    fn build_sub_agent_tools(&self) -> Vec<DynTool> {
        vec![
            Box::new(ReadFileTool::new(self.tools_base_dir.clone())),
            Box::new(ListFilesTool::new(self.tools_base_dir.clone())),
        ]
    }
}

// ---- parsing -----------------------------------------------------------------

impl RunSubagentsTool {
    fn parse_args(&self, args: &str) -> Result<(String, String, Vec<Task>), ExecutionError> {
        let v: serde_json::Value = serde_json::from_str(args)
            .map_err(|e| ExecutionError::args_schema(format!("Invalid JSON args: {}", e)))?;

        let system_prompt = v
            .get("system_prompt")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                ExecutionError::args_schema("Missing required field 'system_prompt'".into())
            })?
            .to_string();

        let model = v
            .get("model")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or(&self.default_model)
            .to_string();

        let tasks = v
            .get("tasks")
            .and_then(|v| v.as_array())
            .ok_or_else(|| ExecutionError::args_schema("Missing required field 'tasks'".into()))?;

        if tasks.is_empty() {
            return Err(ExecutionError::args_schema(
                "'tasks' array must not be empty".into(),
            ));
        }
        if tasks.len() > self.max_tasks {
            return Err(ExecutionError::exec_fail(format!(
                "Too many tasks: {} (max {})",
                tasks.len(),
                self.max_tasks
            )));
        }

        let parsed = tasks
            .iter()
            .enumerate()
            .map(|(i, t)| Self::parse_single_task(i, t))
            .collect::<Result<Vec<_>, _>>()?;

        Ok((model, system_prompt, parsed))
    }

    fn parse_single_task(i: usize, t: &serde_json::Value) -> Result<Task, ExecutionError> {
        let id = t
            .get("id")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                ExecutionError::args_schema(format!("Task {}: missing or empty 'id' field", i))
            })?
            .to_string();

        let prompt = t
            .get("prompt")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                ExecutionError::args_schema(format!(
                    "Task '{}': missing or empty 'prompt' field",
                    id
                ))
            })?
            .to_string();

        Ok(Task { id, prompt })
    }
}

struct Task {
    id: String,
    prompt: String,
}

// ---- orchestration ----------------------------------------------------------

impl RunSubagentsTool {
    async fn spawn_sub_agents(
        &self,
        model: &str,
        system_prompt: &str,
        tasks: Vec<Task>,
    ) -> HashMap<String, Option<String>> {
        let model = model.to_string();
        let system_prompt = system_prompt.to_string();

        let mut join_set = tokio::task::JoinSet::new();

        for task in tasks {
            let model = model.clone();
            let system_prompt = system_prompt.clone();
            let tools = self.build_sub_agent_tools();

            join_set.spawn(async move {
                let result = run_sub_agent(&model, &system_prompt, &task.prompt, tools).await;
                (task.id, result)
            });
        }

        let mut results = HashMap::new();
        while let Some(res) = join_set.join_next().await {
            match res {
                Ok((id, result)) => {
                    results.insert(id, result);
                }
                Err(e) => {
                    tracing::error!(error = %e, "sub-agent task panicked");
                }
            }
        }

        results
    }
}

fn format_results(task_ids: &[String], mut results: HashMap<String, Option<String>>) -> String {
    let mut output = String::from("## Sub-agent Results\n");

    for id in task_ids {
        let result = results.remove(id);

        output.push_str(&format!("\n### {}\n\n", id));
        match result {
            Some(Some(text)) => output.push_str(&text),
            Some(None) => output.push_str("FAILED: resolver error"),
            None => output.push_str("FAILED: task not found"),
        }
        output.push('\n');
    }

    output
}

// ---- provider dispatch ------------------------------------------------------

/// Provider-agnostic sub-agent runner. Dispatches on `model` to build the
/// appropriate Agent, runs it to completion, and returns the final text.
///
/// No generics leak onto `RunSubagentsTool` — the free function encapsulates
/// all provider-specific types inside a per-model `match` arm.
/// Ad-hoc plugin that only provides tools (no interceptor / system prompt).
struct ToolsPlugin {
    tools: Vec<DynTool>,
}

impl<M, R, S, A> IAgentPlugin<M, R, S, A> for ToolsPlugin
where
    M: IMessage + Send + Sync + 'static,
    R: IResolver<Message = M> + Send,
    S: Send + Sync + 'static,
    A: Default + Send + Sync + 'static,
{
    fn tools(&mut self) -> Vec<DynTool> {
        std::mem::take(&mut self.tools)
    }
}

async fn run_sub_agent(
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
    tools: Vec<DynTool>,
) -> Option<String> {
    match model {
        "deepseek-v4-flash" => {
            let resolver = OpenAiResolver::from_env();

            let cx = ContextBuilder::new(model)
                .messages(vec![ChatCompletionMessageParam::System {
                    content: system_prompt.to_string(),
                    name: None,
                }])
                .build();

            let mut agent = OpenAiAgentBuilder::new(cx, resolver)
                .plugin(ToolsPlugin { tools })
                .build();

            agent
                .evaluate(ChatCompletionMessageParam::User {
                    content: UserContent::Text(user_prompt.to_string()),
                    name: None,
                })
                .await
        }
        other => {
            tracing::warn!(model = other, "unsupported model for sub-agent");
            None
        }
    }
}

#[async_trait::async_trait]
impl ITool for RunSubagentsTool {
    fn defination(&self) -> ToolDefination {
        let params = ParamDef::new("object")
            .with_properties(vec![
                (
                    "system_prompt",
                    PropDef::String {
                        desc: "Shared system prompt injected into every sub-agent.".to_string(),
                        r#enum: None,
                    },
                ),
                (
                    "model",
                    PropDef::String {
                        desc: "Model name used to select the underlying agent provider. \
                               Defaults to 'deepseek-v4-flash'."
                            .to_string(),
                        r#enum: Some(vec!["deepseek-v4-flash".to_string()]),
                    },
                ),
                (
                    "tasks",
                    PropDef::Array {
                        desc: "Array of task objects, each with 'id' and 'prompt'. Max 5 tasks."
                            .to_string(),
                    },
                ),
            ])
            .with_required(vec!["system_prompt".to_string(), "tasks".to_string()]);

        ToolDefination::new(
            "run_subagents",
            "Delegate multiple independent tasks to sub-agents that run in parallel. \
             Each sub-agent resolves independently and results are collected. \
             Sub-agents have access to the list_files and read_file tools.",
            params,
        )
        .with_strict(true)
    }

    async fn execute(&mut self, args: &str) -> ExecutionResult {
        let (model, system_prompt, tasks) = self.parse_args(args)?;
        let task_ids: Vec<String> = tasks.iter().map(|t| t.id.clone()).collect();

        let results = self.spawn_sub_agents(&model, &system_prompt, tasks).await;

        Ok(format_results(&task_ids, results))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_definition_is_correct() {
        let tool = RunSubagentsTool::new("deepseek-v4-flash".into(), 5, PathBuf::from("/tmp"));
        let def = tool.defination();

        assert_eq!(def.name, "run_subagents");
        assert_eq!(def.strict, Some(true));
        assert!(def.parameters.props.contains_key("system_prompt"));
        assert!(def.parameters.props.contains_key("model"));
        assert!(def.parameters.props.contains_key("tasks"));
        assert_eq!(
            def.parameters.required,
            Some(vec!["system_prompt".to_string(), "tasks".to_string()])
        );
    }

    #[tokio::test]
    async fn reject_missing_tasks() {
        let mut tool = RunSubagentsTool::new("deepseek-v4-flash".into(), 5, PathBuf::from("/tmp"));

        let result = tool.execute(r#"{"system_prompt":"test"}"#).await;
        assert!(result.is_err(), "missing tasks should be rejected");
        let err = format!("{:?}", result.unwrap_err());
        assert!(err.contains("tasks"), "error should mention tasks: {err}");
    }

    #[tokio::test]
    async fn reject_empty_tasks() {
        let mut tool = RunSubagentsTool::new("deepseek-v4-flash".into(), 5, PathBuf::from("/tmp"));

        let result = tool.execute(r#"{"system_prompt":"test","tasks":[]}"#).await;
        assert!(result.is_err(), "empty tasks should be rejected");
    }

    #[tokio::test]
    async fn reject_too_many_tasks() {
        let mut tool = RunSubagentsTool::new("deepseek-v4-flash".into(), 3, PathBuf::from("/tmp"));

        let result = tool
            .execute(r#"{"system_prompt":"x","tasks":[{"id":"a","prompt":"1"},{"id":"b","prompt":"2"},{"id":"c","prompt":"3"},{"id":"d","prompt":"4"}]}"#)
            .await;
        assert!(result.is_err(), "too many tasks should be rejected");
        let err = format!("{:?}", result.unwrap_err());
        assert!(err.contains("max"), "error should mention max: {err}");
    }

    #[tokio::test]
    async fn reject_task_missing_id_or_prompt() {
        let mut tool = RunSubagentsTool::new("deepseek-v4-flash".into(), 5, PathBuf::from("/tmp"));

        // Missing id
        let result = tool
            .execute(r#"{"system_prompt":"x","tasks":[{"prompt":"hello"}]}"#)
            .await;
        assert!(result.is_err(), "missing id should be rejected");

        // Missing prompt
        let result = tool
            .execute(r#"{"system_prompt":"x","tasks":[{"id":"a"}]}"#)
            .await;
        assert!(result.is_err(), "missing prompt should be rejected");

        // Empty id
        let result = tool
            .execute(r#"{"system_prompt":"x","tasks":[{"id":"","prompt":"hello"}]}"#)
            .await;
        assert!(result.is_err(), "empty id should be rejected");
    }

    #[tokio::test]
    async fn unsupported_model_returns_failed() {
        dotenvy::dotenv().ok();

        let mut tool = RunSubagentsTool::new("deepseek-v4-flash".into(), 5, PathBuf::from("/tmp"));

        let result = tool
            .execute(r#"{"system_prompt":"You are helpful.","model":"nonexistent","tasks":[{"id":"a","prompt":"Say hello."}]}"#)
            .await;

        assert!(
            result.is_ok(),
            "tool should succeed even with unsupported model"
        );
        let output = result.unwrap();
        assert!(output.contains("### a"), "output should contain task id");
        assert!(
            output.contains("FAILED"),
            "unsupported model should produce FAILED, got: {output}"
        );
    }

    #[tokio::test]
    async fn single_subagent_trivial() {
        dotenvy::dotenv().ok();
        let _ = tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .try_init();

        let mut tool = RunSubagentsTool::new("deepseek-v4-flash".into(), 5, PathBuf::from("/tmp"));

        let result = tool
            .execute(
                r#"{"system_prompt":"You are a helpful assistant. Answer concisely.",
                    "tasks":[{"id":"greeting","prompt":"Reply with exactly: hello from sub-agent"}]}"#,
            )
            .await;

        assert!(
            result.is_ok(),
            "single sub-agent should succeed: {:?}",
            result
        );
        let output = result.unwrap();
        assert!(
            output.contains("### greeting"),
            "output should contain task id, got: {output}"
        );
        assert!(
            output.contains("hello from sub-agent"),
            "output should contain sub-agent reply, got: {output}"
        );
        assert!(
            !output.contains("FAILED"),
            "output should not contain FAILED: {output}"
        );
    }

    #[tokio::test]
    async fn multiple_subagents_parallel() {
        dotenvy::dotenv().ok();
        let _ = tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .try_init();

        let mut tool = RunSubagentsTool::new("deepseek-v4-flash".into(), 5, PathBuf::from("/tmp"));

        let result = tool
            .execute(
                r#"{"system_prompt":"You are a helpful assistant. Answer with just the word.",
                    "tasks":[
                      {"id":"alpha","prompt":"Which Greek letter comes after the first one? Reply with just the word."},
                      {"id":"number","prompt":"Reply with exactly: forty-two"}
                    ]}"#,
            )
            .await;

        assert!(
            result.is_ok(),
            "parallel sub-agents should succeed: {:?}",
            result
        );
        let output = result.unwrap();
        assert!(
            output.contains("### alpha"),
            "output should contain alpha task: {output}"
        );
        assert!(
            output.contains("### number"),
            "output should contain number task: {output}"
        );
        assert!(
            !output.contains("FAILED"),
            "output should not contain FAILED: {output}"
        );
    }
}
