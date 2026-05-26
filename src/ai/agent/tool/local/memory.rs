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
