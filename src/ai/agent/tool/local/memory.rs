use std::path::PathBuf;

use serde::Deserialize;

use crate::ai::agent::tool::ITool;
use crate::ai::agent::tool::result::{ToolError, ToolResult};
use crate::ai::resolver::tool::{ParamDef, PropDef, ToolDef};

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Parsed front-matter of a shard.md file.
#[derive(Debug, Deserialize)]
struct ShardMeta {
    name: String,
    description: String,
    tags: Vec<String>,
}

/// Parse `---\n...\n---` YAML front-matter from markdown text.
///
/// Returns `(ShardMeta, body_after_frontmatter)`.
fn parse_frontmatter(raw: &str) -> Result<(ShardMeta, String), String> {
    let mut sections = raw.splitn(3, "---");
    let _empty_before = sections.next(); // skip leading empty
    let yaml_block = sections
        .next()
        .ok_or_else(|| "missing YAML front-matter start `---`".to_string())?;
    let body = sections
        .next()
        .ok_or_else(|| "missing YAML front-matter end `---`".to_string())?;

    let meta: ShardMeta = serde_yaml::from_str(yaml_block.trim())
        .map_err(|e| format!("invalid YAML front-matter: {e}"))?;

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
    fn def(&self) -> ToolDef {
        ToolDef::new(
            "list_memory_shards",
            "List all available memory shards with their name, description, and tags. \
             Call this first to discover which shards exist, then use recall_memory_shard \
             to load the full content of a specific shard.",
            ParamDef::new("object"),
        )
    }

    async fn exec(&mut self, _args: &str) -> ToolResult {
        let mut shards = Vec::new();

        if !self.shards_dir.exists() {
            return Ok("No memory shards directory found.".to_string());
        }

        let dir = std::fs::read_dir(&self.shards_dir)
            .map_err(|e| ToolError::exec_fail(format!("Failed to read shards directory: {e}")))?;

        for entry in dir {
            let entry = entry.map_err(|e| {
                ToolError::exec_fail(format!("Failed to read directory entry: {e}"))
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
                ToolError::exec_fail(format!("Failed to read shard file {:?}: {e}", shard_file))
            })?;

            let (meta, _body) = parse_frontmatter(&raw).map_err(|e| {
                ToolError::exec_fail(format!("Failed to parse {:?}: {e}", shard_file))
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
    fn def(&self) -> ToolDef {
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

        ToolDef::new(
            "recall_memory_shard",
            "Recall the full content of a specific memory shard by name. \
             Use list_memory_shards first to discover available shard names.",
            params,
        )
        .with_strict(true)
    }

    async fn exec(&mut self, args: &str) -> ToolResult {
        let v: serde_json::Value = serde_json::from_str(args)
            .map_err(|e| ToolError::args_schema(format!("Invalid JSON args: {e}")))?;

        let shard_name = v
            .get("shard_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::args_schema("Missing required field 'shard_name'".into()))?;

        let shard_file = self.shards_dir.join(shard_name).join("shard.md");

        let raw = std::fs::read_to_string(&shard_file).map_err(|e| {
            ToolError::exec_fail(format!("Failed to read shard '{shard_name}': {e}"))
        })?;

        let (_meta, body) = parse_frontmatter(&raw).map_err(|e| {
            ToolError::exec_fail(format!("Failed to parse shard '{shard_name}': {e}"))
        })?;

        Ok(body)
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
    fn def(&self) -> ToolDef {
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
                        desc: "Short description of the shard's content"
                            .to_string(),
                        r#enum: None,
                    },
                ),
                (
                    "tags",
                    PropDef::String {
                        desc: "Comma-separated tags for categorization (e.g. 'frontend,translation')"
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

        ToolDef::new(
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

    async fn exec(&mut self, args: &str) -> ToolResult {
        let v: serde_json::Value = serde_json::from_str(args)
            .map_err(|e| ToolError::args_schema(format!("Invalid JSON args: {e}")))?;

        let shard_name = v
            .get("shard_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ToolError::args_schema("Missing required field 'shard_name'".into())
            })?;

        let display_name = v
            .get("display_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ToolError::args_schema("Missing required field 'display_name'".into())
            })?;

        let description = v
            .get("description")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ToolError::args_schema("Missing required field 'description'".into())
            })?;

        let tags = v
            .get("tags")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::args_schema("Missing required field 'tags'".into()))?;

        let content = v
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::args_schema("Missing required field 'content'".into()))?;

        // Validate content length against the 1024-character limit.
        if content.len() > 1024 {
            return Err(ToolError::exec_fail(format!(
                "Content exceeds 1024 character limit (actual: {} characters). \
                 Please shorten the content and try again.",
                content.len()
            )));
        }

        // Build YAML frontmatter.
        let tags_list: Vec<&str> = tags.split(',').map(|t| t.trim()).filter(|t| !t.is_empty()).collect();
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
            ToolError::exec_fail(format!(
                "Failed to create shard directory '{}': {}",
                shard_name, e
            ))
        })?;

        let shard_file = shard_dir.join("shard.md");
        std::fs::write(&shard_file, &shard_content).map_err(|e| {
            ToolError::exec_fail(format!("Failed to write shard file: {}", e))
        })?;

        Ok(format!(
            "Successfully created memory shard '{}' at memory/shards/{}/shard.md ({} characters)",
            shard_name, shard_name, content.len()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_definition_is_correct() {
        let tool = GenerateMemoryShardTool::new(PathBuf::from("/tmp"));
        let def = tool.def();

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
        let result = tool.exec(args).await;

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
        let result = tool.exec(&args).await;

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

        let result = tool.exec(r#"{"shard_name":"test"}"#).await;
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
        let result = tool.exec(&args).await;

        assert!(result.is_ok(), "1024 chars exactly should be accepted: {:?}", result);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
