use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Context as AnyhowContext;
use serde::Deserialize;

use crate::bot::agent::plugin::inspiration::input::MatchInput;
use crate::bot::agent::plugin::inspiration::state::InspiredState;

#[derive(Debug, Deserialize)]
struct KnowledgeItem {
    id: String,
    pattern: String,
    content: String,
}

#[derive(Debug, Clone)]
pub struct KnowledgeEntry {
    pub id: String,
    pub pattern: String,
    pub content: String,
}

impl KnowledgeEntry {
    fn matches(&self, input: &MatchInput<'_>) -> bool {
        self.pattern
            .split('|')
            .map(str::trim)
            .filter(|pattern| !pattern.is_empty())
            .any(|pattern| input.contains(pattern))
    }
}

// TODO: returns Vec<PathBuf> instead of mutating an argument
fn collect_json_files(dir: &Path, files: &mut Vec<PathBuf>) -> anyhow::Result<()> {
    let mut entries = Vec::new();

    for entry in std::fs::read_dir(dir).with_context(|| {
        format!(
            "failed to read inspiration knowledge dir: {}",
            dir.display()
        )
    })? {
        entries.push(entry?);
    }

    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let path = entry.path();

        if path.is_dir() {
            collect_json_files(&path, files)?;
            continue;
        }

        if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
            files.push(path);
        }
    }

    Ok(())
}

fn path_id(root: &Path, path: &Path) -> anyhow::Result<String> {
    let relative = path.strip_prefix(root).with_context(|| {
        format!(
            "failed to resolve inspiration knowledge path: {}",
            path.display()
        )
    })?;
    let mut parts = Vec::new();

    for component in relative.components() {
        let component = component.as_os_str().to_str().with_context(|| {
            format!(
                "inspiration knowledge path is not valid UTF-8: {}",
                path.display()
            )
        })?;
        parts.push(component.to_string());
    }

    let Some(last) = parts.last_mut() else {
        anyhow::bail!(
            "inspiration knowledge path has no file name: {}",
            path.display()
        );
    };
    let Some(stem) = last.strip_suffix(".json") else {
        anyhow::bail!(
            "inspiration knowledge file does not end with .json: {}",
            path.display()
        );
    };
    *last = stem.to_string();

    Ok(parts.join("."))
}

fn validate_non_empty(value: &str, field: &str, path: &Path, index: usize) -> anyhow::Result<()> {
    if value.trim().is_empty() {
        anyhow::bail!(
            "inspiration knowledge field '{}' is empty in {}[{}]",
            field,
            path.display(),
            index
        );
    }

    Ok(())
}

fn load_file(root: &Path, path: &Path) -> anyhow::Result<Vec<KnowledgeEntry>> {
    let raw = std::fs::read_to_string(path).with_context(|| {
        format!(
            "failed to read inspiration knowledge file: {}",
            path.display()
        )
    })?;
    let items: Vec<KnowledgeItem> = serde_json::from_str(&raw).with_context(|| {
        format!(
            "failed to parse inspiration knowledge JSON: {}",
            path.display()
        )
    })?;
    let prefix = path_id(root, path)?;
    let mut entries = Vec::with_capacity(items.len());

    for (index, item) in items.into_iter().enumerate() {
        validate_non_empty(&item.id, "id", path, index)?;
        validate_non_empty(&item.pattern, "pattern", path, index)?;
        validate_non_empty(&item.content, "content", path, index)?;

        let id = if prefix.is_empty() {
            item.id
        } else {
            format!("{}.{}", prefix, item.id)
        };

        entries.push(KnowledgeEntry {
            id,
            pattern: item.pattern,
            content: item.content,
        });
    }

    Ok(entries)
}

#[derive(Debug, Clone, Default)]
pub struct KnowledgeRegistry {
    entries: Vec<KnowledgeEntry>,
}

impl KnowledgeRegistry {
    pub fn load(memory_dir: PathBuf) -> anyhow::Result<Self> {
        let root = memory_dir.join("inspirations").join("knowledges");
        if !root.exists() {
            return Ok(Self::default());
        }

        let mut files = Vec::new();
        collect_json_files(&root, &mut files)?;

        let mut ids = HashSet::new();
        let mut entries = Vec::new();

        for file in files {
            let loaded_entries = load_file(&root, &file)?;

            for entry in loaded_entries {
                if !ids.insert(entry.id.clone()) {
                    anyhow::bail!("duplicate inspiration knowledge id: {}", entry.id);
                }

                entries.push(entry);
            }
        }

        Ok(Self { entries })
    }

    #[cfg(test)]
    pub fn from_entries(entries: Vec<KnowledgeEntry>) -> Self {
        Self { entries }
    }

    pub fn match_entries(
        &self,
        input: &MatchInput<'_>,
        state: &InspiredState,
    ) -> Vec<KnowledgeEntry> {
        self.entries
            .iter()
            .cloned()
            .filter(|entry| !state.active_knowledge_ids.contains(&entry.id))
            .filter(|entry| entry.matches(input))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use uuid::Uuid;

    use crate::bot::agent::plugin::inspiration::input::MatchInput;
    use crate::bot::agent::plugin::inspiration::knowledge::KnowledgeRegistry;
    use crate::bot::agent::plugin::inspiration::state::InspiredState;

    fn temp_memory_dir() -> PathBuf {
        std::env::temp_dir().join(format!(
            "poprako-b-preview-inspiration-test-{}",
            Uuid::new_v4()
        ))
    }

    fn write_entry(memory_dir: &PathBuf, path: &str, raw: &str) {
        let path = memory_dir
            .join("inspirations")
            .join("knowledges")
            .join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, raw).unwrap();
    }

    #[test]
    fn load_builds_id_from_relative_path_and_file_id() {
        let memory_dir = temp_memory_dir();
        write_entry(
            &memory_dir,
            "a/b/c.json",
            r#"[{"id":"member-a","pattern":"member-name-a","content":"member-a 是一名翻译"}]"#,
        );

        let registry = KnowledgeRegistry::load(memory_dir.clone()).unwrap();
        let input = MatchInput::parse("member-name-a");
        let entries = registry.match_entries(&input, &InspiredState::default());

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "a.b.c.member-a");
        assert_eq!(entries[0].content, "member-a 是一名翻译");

        std::fs::remove_dir_all(memory_dir).unwrap();
    }

    #[test]
    fn load_rejects_duplicate_actual_id() {
        let memory_dir = temp_memory_dir();
        write_entry(
            &memory_dir,
            "member.json",
            r#"[{"id":"lb.same","pattern":"LB","content":"one"}]"#,
        );
        write_entry(
            &memory_dir,
            "member/lb.json",
            r#"[{"id":"same","pattern":"LB","content":"one"}]"#,
        );

        let err = KnowledgeRegistry::load(memory_dir.clone()).unwrap_err();
        assert!(format!("{:?}", err).contains("duplicate inspiration knowledge id"));

        std::fs::remove_dir_all(memory_dir).unwrap();
    }

    #[test]
    fn load_rejects_empty_field() {
        let memory_dir = temp_memory_dir();
        write_entry(
            &memory_dir,
            "member/lb.json",
            r#"[{"id":"lb","pattern":"","content":"LB"}]"#,
        );

        let err = KnowledgeRegistry::load(memory_dir.clone()).unwrap_err();
        assert!(format!("{:?}", err).contains("field 'pattern' is empty"));

        std::fs::remove_dir_all(memory_dir).unwrap();
    }

    #[test]
    fn match_entry_supports_pipe_separated_patterns() {
        let registry = KnowledgeRegistry::from_entries(vec![super::KnowledgeEntry {
            id: "role.huiantianqiong".to_string(),
            pattern: "牛牛|灰暗天穹".to_string(),
            content: "翻译、校对".to_string(),
        }]);
        let input = MatchInput::parse("灰暗天穹在吗");
        let entries = registry.match_entries(&input, &InspiredState::default());

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "role.huiantianqiong");
    }
}
