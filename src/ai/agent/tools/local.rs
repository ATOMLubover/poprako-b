use crate::ai::agent::tools::result::{ToolError, ToolResult};
use crate::ai::resolver::tool::{Parameters, Property, Tool};

pub const COMMAND_LINE_TOOL: &str = "command_line";

pub fn command_line_tool() -> Tool {
    const COMMAND_PROPERTY: &str = "command";

    let params = Parameters::new("object")
        .with_properties(vec![(
            COMMAND_PROPERTY,
            Property::String {
                description: "The command to execute.".to_string(),
                r#enum: None,
            },
        )])
        .with_required(vec![COMMAND_PROPERTY.to_string()]);

    Tool::new(
        COMMAND_LINE_TOOL,
        "Execute a command in the shell(/bin/sh) and return the output.",
        params,
    )
    .with_strict(true)
}

pub async fn run_command_line(args: &str) -> ToolResult {
    // Parse JSON arguments to extract the `command` field.
    let command: String = serde_json::from_str::<serde_json::Value>(args)
        .ok()
        .and_then(|v| v.get("command")?.as_str().map(|s| s.to_string()))
        .unwrap_or_else(|| args.to_string());

    let output = tokio::process::Command::new("sh")
        .arg("-c")
        .arg(&command)
        .output()
        .await
        .map_err(|e| ToolError::Fail(format!("Failed to execute command: {}", e)))?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        Ok(stdout)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        Err(ToolError::Fail(format!(
            "Command failed with status {}: {}",
            output.status, stderr
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_definition_is_correct() {
        let tool = command_line_tool();

        assert_eq!(tool.name, "command_line");
        assert!(tool.description.contains("Execute a command"));
        assert!(tool.description.contains("/bin/sh"));
        assert_eq!(tool.parameters.r#type, "object");
        assert!(tool.parameters.properties.contains_key("command"));
        assert_eq!(tool.parameters.required, Some(vec!["command".to_string()]));
        assert_eq!(tool.strict, Some(true));
    }

    #[tokio::test]
    async fn run_successful_command() {
        let result = run_command_line("printf '%s' hello").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "hello");
    }

    #[tokio::test]
    async fn run_failing_command() {
        let result = run_command_line("exit 1").await;
        assert!(result.is_err());
        match result.unwrap_err() {
            ToolError::Fail(msg) => {
                assert!(msg.contains("Command failed"), "msg: {}", msg);
            }
            ToolError::UserAbort => panic!("expected ToolError::Fail"),
        }
    }

    #[tokio::test]
    async fn run_nonexistent_command() {
        let result = run_command_line("nonexistent_command_xyz_123").await;
        assert!(result.is_err());
        match result.unwrap_err() {
            ToolError::Fail(msg) => {
                assert!(msg.contains("Command failed"), "msg: {}", msg);
            }
            ToolError::UserAbort => panic!("expected ToolError::Fail"),
        }
    }

    #[tokio::test]
    async fn run_command_with_stderr_output() {
        let result = run_command_line("echo ok && echo err >&2").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().trim(), "ok");
    }
}
