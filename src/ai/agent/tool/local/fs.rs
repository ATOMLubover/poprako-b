use std::path::{Component, Path, PathBuf};

use crate::ai::agent::tool::ITool;
use crate::ai::agent::tool::result::{ToolError, ToolResult};
use crate::ai::resolver::tool::{ParamDef, PropDef, ToolDef};

pub struct CreateFileTool {
    /// base_dir prevents the tool from creating files outside of this directory.
    base_dir: PathBuf,
    created_files: Vec<PathBuf>,
}

impl CreateFileTool {
    const TOOL_NAME: &'static str = "create_file";

    pub fn new(base_dir: PathBuf) -> Self {
        Self {
            base_dir,
            created_files: Vec::new(),
        }
    }

    pub fn clean_created_files(&mut self) {
        for path in &self.created_files {
            if path.is_file() {
                let _ = std::fs::remove_file(path);
            }
        }

        self.created_files.clear();
    }
}

#[async_trait::async_trait]
impl ITool for CreateFileTool {
    fn def(&self) -> ToolDef {
        let params = ParamDef::new("object")
            .with_properties(vec![
                (
                    "path",
                    PropDef::String {
                        desc:
                            "Relative path of the file to create, relative to the base directory."
                                .to_string(),
                        r#enum: None,
                    },
                ),
                (
                    "content",
                    PropDef::String {
                        desc: "The content to write into the file.".to_string(),
                        r#enum: None,
                    },
                ),
            ])
            .with_required(vec!["path".to_string(), "content".to_string()]);

        ToolDef::new(
            Self::TOOL_NAME,
            "Create a file with the specified content at the given relative path.",
            params,
        )
        .with_strict(true)
    }

    async fn exec(&mut self, args: &str) -> ToolResult {
        let v: serde_json::Value = serde_json::from_str(args)
            .map_err(|e| ToolError::args_schema(format!("Invalid JSON args: {}", e)))?;

        let path = v
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::args_schema("Missing required field 'path'".into()))?;

        let content = v
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::args_schema("Missing required field 'content'".into()))?;

        // Security: reject path traversal.
        let _ = check_path_traversal(path)?;
        let full_path = self.base_dir.join(path);

        // Create parent directories if needed.
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                ToolError::exec_fail(format!("Failed to create parent directories: {e}"))
            })?;
        }

        // Write the file.
        std::fs::write(&full_path, content)
            .map_err(|e| ToolError::exec_fail(format!("Failed to write file: {e}")))?;

        self.created_files.push(full_path);

        Ok(format!("Created file: {path}"))
    }
}

pub struct ListFilesTool {
    base_dir: PathBuf,
}

impl ListFilesTool {
    const TOOL_NAME: &'static str = "list_files";

    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }
}

#[async_trait::async_trait]
impl ITool for ListFilesTool {
    fn def(&self) -> ToolDef {
        let params = ParamDef::new("object")
            .with_properties(vec![(
                "path",
                PropDef::String {
                    desc: "Relative path to list. Use empty string or '.' for the base directory. \
                           This path is always relative to the sandboxed base directory — you \
                           cannot escape it or access files outside."
                        .to_string(),
                    r#enum: None,
                },
            )])
            .with_required(vec!["path".to_string()]);

        ToolDef::new(
            Self::TOOL_NAME,
            "List files and directories at the given relative path. \
             The path is always scoped to the base directory — path traversal (..) is blocked. \
             Directories are marked with a trailing '/'.",
            params,
        )
        .with_strict(true)
    }

    async fn exec(&mut self, args: &str) -> ToolResult {
        let v: serde_json::Value = serde_json::from_str(args)
            .map_err(|e| ToolError::args_schema(format!("Invalid JSON args: {e}")))?;

        let path = v
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or(".");

        // An empty string defaults to base dir.
        let path = if path.is_empty() { "." } else { path };

        let _ = check_path_traversal(path)?;
        let full_path = self.base_dir.join(path);

        let entries = std::fs::read_dir(&full_path).map_err(|e| {
            ToolError::exec_fail(format!("Failed to read directory '{}': {e}", path))
        })?;

        let mut listing = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|e| {
                ToolError::exec_fail(format!("Failed to read entry in '{}': {e}", path))
            })?;

            let name = entry.file_name();
            let name = name.to_string_lossy();

            let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
            if is_dir {
                listing.push(format!("  {name}/ (directory)"));
            } else {
                listing.push(format!("  {name}"));
            }
        }

        if listing.is_empty() {
            Ok(format!("(empty directory)"))
        } else {
            listing.sort();
            Ok(listing.join("\n"))
        }
    }
}

pub struct ReadFileTool {
    base_dir: PathBuf,
}

impl ReadFileTool {
    const TOOL_NAME: &'static str = "read_file";

    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }
}

/// Shared path-traversal check used by both local tools.
fn check_path_traversal(path: &str) -> Result<PathBuf, ToolError> {
    let relative = Path::new(path);
    if relative
        .components()
        .any(|c| matches!(c, Component::ParentDir))
    {
        return Err(ToolError::exec_fail(
            "Path traversal not allowed: path must not contain '..'".into(),
        ));
    }
    Ok(relative.to_path_buf())
}

#[async_trait::async_trait]
impl ITool for ReadFileTool {
    fn def(&self) -> ToolDef {
        let params = ParamDef::new("object")
            .with_properties(vec![(
                "path",
                PropDef::String {
                    desc: "Relative path of the file to read, relative to the base directory."
                        .to_string(),
                    r#enum: None,
                },
            )])
            .with_required(vec!["path".to_string()]);

        ToolDef::new(
            Self::TOOL_NAME,
            "Read the contents of a file at the given relative path.",
            params,
        )
        .with_strict(true)
    }

    async fn exec(&mut self, args: &str) -> ToolResult {
        let v: serde_json::Value = serde_json::from_str(args)
            .map_err(|e| ToolError::args_schema(format!("Invalid JSON args: {e}")))?;

        let path = v
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::args_schema("Missing required field 'path'".into()))?;

        let _ = check_path_traversal(path)?;
        let full_path = self.base_dir.join(path);

        let content = std::fs::read_to_string(&full_path)
            .map_err(|e| ToolError::exec_fail(format!("Failed to read file: {e}")))?;

        Ok(content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_definition_is_correct() {
        let tool = CreateFileTool::new(PathBuf::from("/tmp"));
        let def = tool.def();

        assert_eq!(def.name, "create_file");
        assert_eq!(def.strict, Some(true));
        assert!(def.parameters.props.contains_key("path"));
        assert!(def.parameters.props.contains_key("content"));
        assert_eq!(
            def.parameters.required,
            Some(vec!["path".to_string(), "content".to_string()])
        );
    }

    #[tokio::test]
    async fn create_file_success() {
        let dir = std::env::temp_dir().join("poprako-test-create-file");
        let target = dir.join("sub/hello.txt");

        // Clean up any leftovers from previous runs.
        let _ = std::fs::remove_dir_all(&dir);

        let mut tool = CreateFileTool::new(dir.clone());
        let args = r#"{"path":"sub/hello.txt","content":"hello world"}"#;
        let result = tool.exec(args).await;

        assert!(result.is_ok(), "execute should succeed: {:?}", result);
        assert!(target.exists(), "file should exist at {}", target.display());

        let contents = std::fs::read_to_string(&target).expect("should read file");
        assert_eq!(contents, "hello world");

        // Clean up via the tool's own interface.
        tool.clean_created_files();
        assert!(!target.exists(), "file should be removed after cleanup");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn reject_path_traversal() {
        let dir = std::env::temp_dir().join("poprako-test-traversal");
        let mut tool = CreateFileTool::new(dir);

        let result = tool
            .exec(r#"{"path":"../../etc/passwd","content":"pwned"}"#)
            .await;
        assert!(result.is_err(), "path traversal should be rejected");
    }

    #[tokio::test]
    async fn reject_missing_field() {
        let dir = std::env::temp_dir().join("poprako-test-missing");
        let mut tool = CreateFileTool::new(dir);

        let result = tool.exec(r#"{"path":"test.txt"}"#).await;
        assert!(result.is_err(), "missing content should be rejected");

        let result = tool.exec(r#"{"content":"test"}"#).await;
        assert!(result.is_err(), "missing path should be rejected");
    }

    // ---- ListFilesTool tests ----

    #[test]
    fn list_files_tool_definition_is_correct() {
        let tool = ListFilesTool::new(PathBuf::from("/tmp"));
        let def = tool.def();

        assert_eq!(def.name, "list_files");
        assert_eq!(def.strict, Some(true));
        assert!(def.parameters.props.contains_key("path"));
        assert_eq!(def.parameters.required, Some(vec!["path".to_string()]));
    }

    #[tokio::test]
    async fn list_files_root() {
        let dir = std::env::temp_dir().join("poprako-test-list-files");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create dir");

        // Create some files and a subdirectory.
        std::fs::write(dir.join("a.txt"), "aaa").expect("write a.txt");
        std::fs::write(dir.join("b.txt"), "bbb").expect("write b.txt");
        std::fs::create_dir(dir.join("sub")).expect("create sub dir");

        let mut tool = ListFilesTool::new(dir.clone());

        let result = tool.exec(r#"{"path":"."}"#).await;
        assert!(result.is_ok(), "list should succeed: {:?}", result);
        let output = result.unwrap();

        assert!(output.contains("a.txt"), "should list a.txt: {output}");
        assert!(output.contains("b.txt"), "should list b.txt: {output}");
        assert!(
            output.contains("sub/ (directory)"),
            "should list sub directory: {output}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn list_files_subdirectory() {
        let dir = std::env::temp_dir().join("poprako-test-list-sub");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sub")).expect("create sub");
        std::fs::write(dir.join("sub/nested.txt"), "nested").expect("write nested");

        let mut tool = ListFilesTool::new(dir.clone());

        let result = tool.exec(r#"{"path":"sub"}"#).await;
        assert!(
            result.is_ok(),
            "list sub should succeed: {:?}",
            result
        );
        let output = result.unwrap();
        assert!(
            output.contains("nested.txt"),
            "should list nested.txt: {output}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn list_files_empty_directory() {
        let dir = std::env::temp_dir().join("poprako-test-list-empty");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create dir");

        let mut tool = ListFilesTool::new(dir.clone());

        let result = tool.exec(r#"{"path":"."}"#).await;
        assert!(result.is_ok(), "list empty should succeed");
        assert_eq!(result.unwrap(), "(empty directory)");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn list_files_empty_path_defaults_to_root() {
        let dir = std::env::temp_dir().join("poprako-test-list-empty-path");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create dir");
        std::fs::write(dir.join("x.txt"), "x").expect("write");

        let mut tool = ListFilesTool::new(dir.clone());
        let result = tool.exec(r#"{"path":""}"#).await;

        assert!(result.is_ok(), "empty path should default to root");
        assert!(result.unwrap().contains("x.txt"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn list_files_rejects_traversal() {
        let dir = std::env::temp_dir().join("poprako-test-list-traversal");
        let mut tool = ListFilesTool::new(dir);

        let result = tool.exec(r#"{"path":"../../etc"}"#).await;
        assert!(result.is_err(), "path traversal should be rejected");
    }

    #[tokio::test]
    async fn list_files_nonexistent_directory() {
        let dir = std::env::temp_dir().join("poprako-test-list-nonexistent");
        let mut tool = ListFilesTool::new(dir);

        let result = tool.exec(r#"{"path":"nope"}"#).await;
        assert!(result.is_err(), "nonexistent dir should fail");
    }

    // ---- ReadFileTool tests ----

    #[test]
    fn read_file_tool_definition_is_correct() {
        let tool = ReadFileTool::new(PathBuf::from("/tmp"));
        let def = tool.def();

        assert_eq!(def.name, "read_file");
        assert_eq!(def.strict, Some(true));
        assert!(def.parameters.props.contains_key("path"));
        assert_eq!(def.parameters.required, Some(vec!["path".to_string()]));
    }

    #[tokio::test]
    async fn read_file_success() {
        let dir = std::env::temp_dir().join("poprako-test-read-file");
        let target = dir.join("test.txt");

        // Set up: create a file to read.
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create dir");
        std::fs::write(&target, "hello read").expect("write file");

        let mut tool = ReadFileTool::new(dir.clone());
        let result = tool.exec(r#"{"path":"test.txt"}"#).await;

        assert!(result.is_ok(), "read should succeed: {:?}", result);
        assert_eq!(result.unwrap(), "hello read");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn read_file_nonexistent() {
        let dir = std::env::temp_dir().join("poprako-test-read-nonexistent");
        let mut tool = ReadFileTool::new(dir);

        let result = tool.exec(r#"{"path":"nope.txt"}"#).await;
        assert!(result.is_err(), "reading nonexistent file should fail");
    }

    #[tokio::test]
    async fn read_file_rejects_traversal() {
        let dir = std::env::temp_dir().join("poprako-test-read-traversal");
        let mut tool = ReadFileTool::new(dir);

        let result = tool.exec(r#"{"path":"../../etc/passwd"}"#).await;
        assert!(result.is_err(), "path traversal should be rejected");
    }
}
