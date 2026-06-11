use std::path::PathBuf;

use serde::Deserialize;

use crate::ai::agent::tool::ITool;
use crate::ai::agent::tool::result::{ExecutionError, ExecutionResult};
use crate::ai::resolver::tool::{ParamDef, PropDef, ToolDefination};

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Parsed front-matter of a shard.md file.
#[derive(Debug, Deserialize)]
pub struct ShardMeta {
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
}

/// Parse `---\n...\n---` YAML front-matter from markdown text.
///
/// Returns `(ShardMeta, body_after_frontmatter)`.
pub fn parse_frontmatter(raw: &str) -> Result<(ShardMeta, String), String> {
    let mut sections = raw.splitn(3, "---");
    let _empty_before = sections.next(); // skip leading empty
    let yaml_block = sections
        .next()
        .ok_or_else(|| "missing YAML front-matter start `---`".to_string())?;
    let body = sections
        .next()
        .ok_or_else(|| "missing YAML front-matter end `---`".to_string())?;

    let meta: ShardMeta = serde_yaml::from_str(yaml_block.trim())
        .map_err(|e| format!("invalid YAML front-matter: {}", e))?;

    Ok((meta, body.to_string()))
}

// ---------------------------------------------------------------------------
// ListMemoryShardsTool
// ---------------------------------------------------------------------------

pub struct ListMemoryShardsTool {
    shards_dir: PathBuf,
}

impl ListMemoryShardsTool {
    pub fn new(memory_dir: PathBuf) -> Self {
        Self {
            shards_dir: memory_dir.join("shards"),
        }
    }
}

#[async_trait::async_trait]
impl ITool for ListMemoryShardsTool {
    fn defination(&self) -> ToolDefination {
        ToolDefination::new(
            "list_memory_shards",
            "List all available memory shards with their name, description, and tags. \
             Call this first to discover which shards exist, then use recall_memory_shard \
             to load the full content of a specific shard.",
            ParamDef::new("object"),
        )
    }

    async fn execute(&mut self, _args: &str) -> ExecutionResult {
        let mut shards = Vec::new();

        if !self.shards_dir.exists() {
            return Ok("No memory shards directory found.".to_string());
        }

        let dir = std::fs::read_dir(&self.shards_dir).map_err(|e| {
            ExecutionError::exec_fail(format!("Failed to read shards directory: {}", e))
        })?;

        for entry in dir {
            let entry = entry.map_err(|e| {
                ExecutionError::exec_fail(format!("Failed to read directory entry: {}", e))
            })?;

            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let shard_file = path.join("shard.md");
            if !shard_file.exists() {
                continue;
            }

            let raw = std::fs::read_to_string(&shard_file).map_err(|e| {
                ExecutionError::exec_fail(format!(
                    "Failed to read shard file {:?}: {e}",
                    shard_file
                ))
            })?;

            let (meta, _body) = parse_frontmatter(&raw).map_err(|e| {
                ExecutionError::exec_fail(format!("Failed to parse {:?}: {}", shard_file, e))
            })?;

            shards.push(format!(
                "- **{}**: {} [tags: {}]",
                meta.name,
                meta.description,
                meta.tags.join(", ")
            ));
        }

        if shards.is_empty() {
            Ok("No memory shards found.".to_string())
        } else {
            Ok(shards.join("\n"))
        }
    }
}

// ---------------------------------------------------------------------------
// RecallMemoryShardTool
// ---------------------------------------------------------------------------

pub struct RecallMemoryShardTool {
    shards_dir: PathBuf,
}

impl RecallMemoryShardTool {
    pub fn new(memory_dir: PathBuf) -> Self {
        Self {
            shards_dir: memory_dir.join("shards"),
        }
    }
}

#[async_trait::async_trait]
impl ITool for RecallMemoryShardTool {
    fn defination(&self) -> ToolDefination {
        let params = ParamDef::new("object")
            .with_properties(vec![(
                "shard_name",
                PropDef::String {
                    desc: "Name of the memory shard to recall. Obtain valid names \
                           by calling list_memory_shards first."
                        .to_string(),
                    r#enum: None,
                },
            )])
            .with_required(vec!["shard_name".to_string()]);

        ToolDefination::new(
            "recall_memory_shard",
            "Recall the full content of a specific memory shard by name. \
             Use list_memory_shards first to discover available shard names.",
            params,
        )
        .with_strict(true)
    }

    async fn execute(&mut self, args: &str) -> ExecutionResult {
        let v: serde_json::Value = serde_json::from_str(args)
            .map_err(|e| ExecutionError::args_schema(format!("Invalid JSON args: {}", e)))?;

        let shard_name = v
            .get("shard_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ExecutionError::args_schema("Missing required field 'shard_name'".into())
            })?;

        let shard_file = self.shards_dir.join(shard_name).join("shard.md");

        let raw = std::fs::read_to_string(&shard_file).map_err(|e| {
            ExecutionError::exec_fail(format!("Failed to read shard '{}': {}", shard_name, e))
        })?;

        let (_meta, body) = parse_frontmatter(&raw).map_err(|e| {
            ExecutionError::exec_fail(format!("Failed to parse shard '{}': {}", shard_name, e))
        })?;

        Ok(body.trim_start().to_string())
    }
}

// ---------------------------------------------------------------------------
// GenerateMemoryShardTool
// ---------------------------------------------------------------------------

pub struct GenerateMemoryShardTool {
    shards_dir: PathBuf,
}

impl GenerateMemoryShardTool {
    pub fn new(memory_dir: PathBuf) -> Self {
        Self {
            shards_dir: memory_dir.join("shards"),
        }
    }
}

#[async_trait::async_trait]
impl ITool for GenerateMemoryShardTool {
    fn defination(&self) -> ToolDefination {
        let params = ParamDef::new("object")
            .with_properties(vec![
                (
                    "shard_name",
                    PropDef::String {
                        desc: "Directory name for the shard (kebab-case, e.g. 'my-new-topic')"
                            .to_string(),
                        r#enum: None,
                    },
                ),
                (
                    "display_name",
                    PropDef::String {
                        desc: "Display name for the shard (used in frontmatter 'name' field)"
                            .to_string(),
                        r#enum: None,
                    },
                ),
                (
                    "description",
                    PropDef::String {
                        desc: "Short description of the shard's content".to_string(),
                        r#enum: None,
                    },
                ),
                (
                    "tags",
                    PropDef::String {
                        desc:
                            "Comma-separated tags for categorization (e.g. 'frontend,translation')"
                                .to_string(),
                        r#enum: None,
                    },
                ),
                (
                    "content",
                    PropDef::String {
                        desc: "The body content of the shard in Markdown. Max 1024 characters. \
                               This is the factual knowledge that will be recalled later."
                            .to_string(),
                        r#enum: None,
                    },
                ),
            ])
            .with_required(vec![
                "shard_name".to_string(),
                "display_name".to_string(),
                "description".to_string(),
                "tags".to_string(),
                "content".to_string(),
            ]);

        ToolDefination::new(
            "generate_memory_shard",
            "Generate a new memory shard by creating a shard.md file under memory/shards/. \
             IMPORTANT: You MUST call recall_memory_shard with shard_name='how-to-create-shard' \
             first to learn the shard creation guidelines before using this tool. \
             The body content is limited to 1024 characters; exceeding this will be rejected. \
             Only create shards when explicitly instructed by LB or a developer — never proactively.",
            params,
        )
        .with_strict(true)
    }

    async fn execute(&mut self, args: &str) -> ExecutionResult {
        let v: serde_json::Value = serde_json::from_str(args)
            .map_err(|e| ExecutionError::args_schema(format!("Invalid JSON args: {}", e)))?;

        let shard_name = v
            .get("shard_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ExecutionError::args_schema("Missing required field 'shard_name'".into())
            })?;

        let display_name = v
            .get("display_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ExecutionError::args_schema("Missing required field 'display_name'".into())
            })?;

        let description = v
            .get("description")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ExecutionError::args_schema("Missing required field 'description'".into())
            })?;

        let tags = v
            .get("tags")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ExecutionError::args_schema("Missing required field 'tags'".into()))?;

        let content = v.get("content").and_then(|v| v.as_str()).ok_or_else(|| {
            ExecutionError::args_schema("Missing required field 'content'".into())
        })?;

        // Validate content length against the 1024-character limit.
        if content.len() > 1024 {
            return Err(ExecutionError::exec_fail(format!(
                "Content exceeds 1024 character limit (actual: {} characters). \
                 Please shorten the content and try again.",
                content.len()
            )));
        }

        // Build YAML frontmatter.
        let tags_list: Vec<&str> = tags
            .split(',')
            .map(|t| t.trim())
            .filter(|t| !t.is_empty())
            .collect();
        let tags_yaml = tags_list
            .iter()
            .map(|t| format!("  - {}", t))
            .collect::<Vec<_>>()
            .join("\n");

        let shard_content = format!(
            "---\nname: {}\ndescription: {}\ntags:\n{}\n---\n\n{}",
            display_name, description, tags_yaml, content
        );

        // Create shard directory and write the file.
        let shard_dir = self.shards_dir.join(shard_name);
        std::fs::create_dir_all(&shard_dir).map_err(|e| {
            ExecutionError::exec_fail(format!(
                "Failed to create shard directory '{}': {}",
                shard_name, e
            ))
        })?;

        let shard_file = shard_dir.join("shard.md");
        std::fs::write(&shard_file, &shard_content)
            .map_err(|e| ExecutionError::exec_fail(format!("Failed to write shard file: {}", e)))?;

        Ok(format!(
            "Successfully created memory shard '{}' at memory/shards/{}/shard.md ({} characters)",
            shard_name,
            shard_name,
            content.len()
        ))
    }
}

// ---------------------------------------------------------------------------
// ModifyMemoryShardTool
// ---------------------------------------------------------------------------

pub struct ModifyMemoryShardTool {
    shards_dir: PathBuf,
}

impl ModifyMemoryShardTool {
    pub fn new(memory_dir: PathBuf) -> Self {
        Self {
            shards_dir: memory_dir.join("shards"),
        }
    }
}

#[async_trait::async_trait]
impl ITool for ModifyMemoryShardTool {
    fn defination(&self) -> ToolDefination {
        let params = ParamDef::new("object")
            .with_properties(vec![
                (
                    "shard_name",
                    PropDef::String {
                        desc: "Name of the shard to modify (directory name, kebab-case)"
                            .to_string(),
                        r#enum: None,
                    },
                ),
                (
                    "start_line",
                    PropDef::Number {
                        desc: "The 1-based line number (in the body, after frontmatter) where \
                               replacement begins. Line 1 is the first line after the closing \
                               `---` of the YAML frontmatter. This line will be replaced."
                            .to_string(),
                        r#enum: None,
                    },
                ),
                (
                    "end_line",
                    PropDef::Number {
                        desc: "The 1-based line number (in the body, inclusive) where replacement \
                               ends. Use the same value as start_line to replace a single line. \
                               Use 0 to append after the last line."
                            .to_string(),
                        r#enum: None,
                    },
                ),
                (
                    "new_text",
                    PropDef::String {
                        desc: "The replacement text. For multi-line replacements, use \\n to \
                               separate lines. When end_line is 0 (append mode), new_text is \
                               appended as new lines after the existing body lines."
                            .to_string(),
                        r#enum: None,
                    },
                ),
            ])
            .with_required(vec![
                "shard_name".to_string(),
                "start_line".to_string(),
                "end_line".to_string(),
                "new_text".to_string(),
            ]);

        ToolDefination::new(
            "modify_memory_shard",
            "Modify the body content of an existing memory shard. \
             IMPORTANT: You MUST first call recall_memory_shard to read the shard's current \
             content and determine the exact line numbers before using this tool. \
             Line numbers are 1-based and count from the first line after the YAML frontmatter \
             (after the closing `---`). \
             AFTER calling this tool, you MUST call recall_memory_shard again to verify the \
             modification was applied correctly.",
            params,
        )
        .with_strict(true)
    }

    async fn execute(&mut self, args: &str) -> ExecutionResult {
        let v: serde_json::Value = serde_json::from_str(args)
            .map_err(|e| ExecutionError::args_schema(format!("Invalid JSON args: {}", e)))?;

        let shard_name = v
            .get("shard_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ExecutionError::args_schema("Missing required field 'shard_name'".into())
            })?;

        let start_line = v
            .get("start_line")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| {
                ExecutionError::args_schema("Missing required field 'start_line'".into())
            })? as usize;

        let end_line = v.get("end_line").and_then(|v| v.as_u64()).ok_or_else(|| {
            ExecutionError::args_schema("Missing required field 'end_line'".into())
        })? as usize;

        let new_text = v.get("new_text").and_then(|v| v.as_str()).ok_or_else(|| {
            ExecutionError::args_schema("Missing required field 'new_text'".into())
        })?;

        let shard_file = self.shards_dir.join(shard_name).join("shard.md");

        let raw = std::fs::read_to_string(&shard_file).map_err(|e| {
            ExecutionError::exec_fail(format!("Failed to read shard '{}': {}", shard_name, e))
        })?;

        let (meta, body) = parse_frontmatter(&raw).map_err(|e| {
            ExecutionError::exec_fail(format!("Failed to parse shard '{}': {}", shard_name, e))
        })?;

        // Trim leading whitespace from body (the blank lines between frontmatter and content).
        // Use split('\n') instead of lines() so trailing empty lines are preserved,
        // keeping line numbers consistent with what the LLM sees in recall output.
        let body_trimmed = body.trim_start().to_string();
        let body_lines_ref: Vec<&str> = body_trimmed.split('\n').collect();
        let mut body_lines: Vec<String> = body_lines_ref.iter().map(|s| s.to_string()).collect();

        // Replace newline characters in new_text with actual newlines.
        let new_lines: Vec<&str> = new_text.split("\\n").collect();

        let old_body = body_trimmed;

        if end_line == 0 {
            // Append mode: add new lines after existing body.
            for line in &new_lines {
                body_lines.push(line.to_string());
            }
        } else {
            // Replace range [start_line, end_line] (1-based, inclusive).
            if start_line == 0 || start_line > body_lines.len() + 1 {
                return Err(ExecutionError::exec_fail(format!(
                    "start_line {start_line} is out of range (body has {} lines)",
                    body_lines.len()
                )));
            }
            if end_line < start_line {
                return Err(ExecutionError::exec_fail(format!(
                    "end_line {end_line} must be >= start_line {start_line}"
                )));
            }
            if end_line > body_lines.len() {
                return Err(ExecutionError::exec_fail(format!(
                    "end_line {end_line} is out of range (body has {} lines)",
                    body_lines.len()
                )));
            }

            let start_idx = start_line - 1;
            let end_idx = end_line; // exclusive for drain
            body_lines.drain(start_idx..end_idx);

            // Insert new lines at start_idx, or if start_idx is beyond current
            // length (replacing past last line), append.
            let insert_pos = start_idx.min(body_lines.len());
            for (i, line) in new_lines.iter().enumerate() {
                body_lines.insert(insert_pos + i, line.to_string());
            }
        }

        let new_body = body_lines.join("\n");

        // Enforce 1024 character limit.
        if new_body.len() > 1024 {
            return Err(ExecutionError::exec_fail(format!(
                "Modified body would be {} characters, exceeding the 1024 character limit. \
                 Please shorten the replacement and try again.",
                new_body.len()
            )));
        }

        // Rebuild YAML frontmatter + new body.
        let tags_yaml = meta
            .tags
            .iter()
            .map(|t| format!("  - {}", t))
            .collect::<Vec<_>>()
            .join("\n");

        let shard_content = format!(
            "---\nname: {}\ndescription: {}\ntags:\n{}\n---\n\n{}",
            meta.name, meta.description, tags_yaml, new_body
        );

        std::fs::write(&shard_file, &shard_content)
            .map_err(|e| ExecutionError::exec_fail(format!("Failed to write shard file: {}", e)))?;

        let old_line_count = old_body.split('\n').count();
        let new_line_count = new_body.split('\n').count();

        Ok(format!(
            "Successfully modified shard '{shard_name}'. Body changed from {} to {} lines. \
             REMEMBER: You MUST now call recall_memory_shard with shard_name='{shard_name}' \
             to verify the modification is correct.",
            old_line_count, new_line_count,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::Path;

    #[test]
    fn tool_definition_is_correct() {
        let tool = GenerateMemoryShardTool::new(PathBuf::from("/tmp"));
        let def = tool.defination();

        assert_eq!(def.name, "generate_memory_shard");
        assert_eq!(def.strict, Some(true));
        assert!(def.parameters.props.contains_key("shard_name"));
        assert!(def.parameters.props.contains_key("display_name"));
        assert!(def.parameters.props.contains_key("description"));
        assert!(def.parameters.props.contains_key("tags"));
        assert!(def.parameters.props.contains_key("content"));
        assert_eq!(
            def.parameters.required,
            Some(vec![
                "shard_name".to_string(),
                "display_name".to_string(),
                "description".to_string(),
                "tags".to_string(),
                "content".to_string(),
            ])
        );
    }

    #[tokio::test]
    async fn create_shard_success() {
        let dir = std::env::temp_dir().join("poprako-test-generate-shard");
        let _ = std::fs::remove_dir_all(&dir);

        let mut tool = GenerateMemoryShardTool::new(dir.clone());
        let args = r#"{"shard_name":"test-topic","display_name":"Test Topic","description":"A test shard","tags":"test,example","content":"This is the shard body."}"#;
        let result = tool.execute(args).await;

        assert!(result.is_ok(), "create should succeed: {:?}", result);

        let shard_file = dir.join("shards").join("test-topic").join("shard.md");
        assert!(shard_file.exists(), "shard file should exist");

        let raw = std::fs::read_to_string(&shard_file).unwrap();
        assert!(raw.contains("name: Test Topic"));
        assert!(raw.contains("description: A test shard"));
        assert!(raw.contains("  - test"));
        assert!(raw.contains("  - example"));
        assert!(raw.contains("This is the shard body."));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn reject_content_over_1024_chars() {
        let dir = std::env::temp_dir().join("poprako-test-shard-overflow");
        let _ = std::fs::remove_dir_all(&dir);

        let mut tool = GenerateMemoryShardTool::new(dir.clone());
        let long_content = "x".repeat(1025);
        let args = format!(
            r#"{{"shard_name":"big","display_name":"Big","description":"test","tags":"test","content":"{}"}}"#,
            long_content
        );
        let result = tool.execute(&args).await;

        assert!(result.is_err(), "should reject content over 1024 chars");
        let err = result.unwrap_err();
        let msg = format!("{:?}", err);
        assert!(msg.contains("1024"), "error should mention limit: {}", msg);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn reject_missing_fields() {
        let dir = std::env::temp_dir().join("poprako-test-shard-missing");
        let _ = std::fs::remove_dir_all(&dir);

        let mut tool = GenerateMemoryShardTool::new(dir.clone());

        let result = tool.execute(r#"{"shard_name":"test"}"#).await;
        assert!(result.is_err(), "missing fields should be rejected");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn content_at_exactly_1024_chars_is_accepted() {
        let dir = std::env::temp_dir().join("poprako-test-shard-1024");
        let _ = std::fs::remove_dir_all(&dir);

        let mut tool = GenerateMemoryShardTool::new(dir.clone());
        let exact_content = "x".repeat(1024);
        let args = format!(
            r#"{{"shard_name":"exact","display_name":"Exact","description":"test","tags":"test","content":"{}"}}"#,
            exact_content
        );
        let result = tool.execute(&args).await;

        assert!(
            result.is_ok(),
            "1024 chars exactly should be accepted: {:?}",
            result
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    // -- ModifyMemoryShardTool tests --

    fn create_test_shard(dir: &Path, name: &str, body: &str) {
        let shard_dir = dir.join("shards").join(name);
        std::fs::create_dir_all(&shard_dir).unwrap();
        let content = format!(
            "---\nname: {}\ndescription: test\ntags:\n  - test\n---\n\n{}",
            name, body
        );
        std::fs::write(shard_dir.join("shard.md"), content).unwrap();
    }

    fn read_shard_body(dir: &Path, name: &str) -> String {
        let raw = std::fs::read_to_string(dir.join("shards").join(name).join("shard.md")).unwrap();
        let (_, body) = parse_frontmatter(&raw).unwrap();
        body.trim_start().to_string()
    }

    #[test]
    fn modify_tool_definition_is_correct() {
        let tool = ModifyMemoryShardTool::new(PathBuf::from("/tmp"));
        let def = tool.defination();

        assert_eq!(def.name, "modify_memory_shard");
        assert_eq!(def.strict, Some(true));
        assert!(def.parameters.props.contains_key("shard_name"));
        assert!(def.parameters.props.contains_key("start_line"));
        assert!(def.parameters.props.contains_key("end_line"));
        assert!(def.parameters.props.contains_key("new_text"));
    }

    #[tokio::test]
    async fn modify_replace_single_line() {
        let dir = std::env::temp_dir().join("poprako-test-modify-single");
        let _ = std::fs::remove_dir_all(&dir);
        create_test_shard(&dir, "test", "line1\nline2\nline3");

        let mut tool = ModifyMemoryShardTool::new(dir.clone());
        let args = r#"{"shard_name":"test","start_line":2,"end_line":2,"new_text":"replaced"}"#;
        let result = tool.execute(args).await;

        assert!(result.is_ok(), "modify should succeed: {:?}", result);
        let body = read_shard_body(&dir, "test");
        assert_eq!(body, "line1\nreplaced\nline3");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn modify_replace_range() {
        let dir = std::env::temp_dir().join("poprako-test-modify-range");
        let _ = std::fs::remove_dir_all(&dir);
        create_test_shard(&dir, "test", "a\nb\nc\nd\ne");

        let mut tool = ModifyMemoryShardTool::new(dir.clone());
        let args = r#"{"shard_name":"test","start_line":2,"end_line":4,"new_text":"x\\ny"}"#;
        let result = tool.execute(args).await;

        assert!(result.is_ok(), "modify range should succeed: {:?}", result);
        let body = read_shard_body(&dir, "test");
        assert_eq!(body, "a\nx\ny\ne");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn modify_append() {
        let dir = std::env::temp_dir().join("poprako-test-modify-append");
        let _ = std::fs::remove_dir_all(&dir);
        create_test_shard(&dir, "test", "line1");

        let mut tool = ModifyMemoryShardTool::new(dir.clone());
        let args = r#"{"shard_name":"test","start_line":1,"end_line":0,"new_text":"line2"}"#;
        let result = tool.execute(args).await;

        assert!(result.is_ok(), "append should succeed: {:?}", result);
        let body = read_shard_body(&dir, "test");
        assert_eq!(body, "line1\nline2");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn modify_rejects_overflow() {
        let dir = std::env::temp_dir().join("poprako-test-modify-overflow");
        let _ = std::fs::remove_dir_all(&dir);
        create_test_shard(&dir, "test", "short");

        let mut tool = ModifyMemoryShardTool::new(dir.clone());
        let long = "x".repeat(1025);
        let args =
            format!(r#"{{"shard_name":"test","start_line":1,"end_line":1,"new_text":"{long}"}}"#);
        let result = tool.execute(&args).await;

        assert!(result.is_err(), "should reject overflow");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn modify_rejects_invalid_line_numbers() {
        let dir = std::env::temp_dir().join("poprako-test-modify-invalid-line");
        let _ = std::fs::remove_dir_all(&dir);
        create_test_shard(&dir, "test", "a\nb");

        let mut tool = ModifyMemoryShardTool::new(dir.clone());

        // end_line < start_line
        let args = r#"{"shard_name":"test","start_line":2,"end_line":1,"new_text":"x"}"#;
        let result = tool.execute(args).await;
        assert!(result.is_err());

        // start_line out of range
        let args = r#"{"shard_name":"test","start_line":10,"end_line":10,"new_text":"x"}"#;
        let result = tool.execute(args).await;
        assert!(result.is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
