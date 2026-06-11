use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::Context as _;
use regex::Regex;
use serde::Deserialize;

use crate::bot::agent::plugin::inspiration::input::MatchInput;
use crate::bot::agent::plugin::inspiration::state::InspiredState;

#[derive(Debug, Deserialize)]
struct KnowledgeItem {
    id: String,
    title: String,
    pattern: String,
    content: String,
}

#[derive(Debug, Clone)]
pub struct KnowledgeEntry {
    pub id: String,
    pub namespace: String,
    pub title: String,
    pub content: String,
    pattern_regex: Regex,
}

impl KnowledgeEntry {
    #[cfg(test)]
    pub fn new(
        namespace: impl Into<String>,
        id: impl Into<String>,
        title: impl Into<String>,
        pattern: impl Into<String>,
        content: impl Into<String>,
    ) -> anyhow::Result<Self> {
        let namespace = namespace.into();
        let id = id.into();
        let pattern = pattern.into();
        let pattern_regex = Regex::new(&pattern).with_context(|| {
            format!(
                "inspiration knowledge pattern is not a valid regex: {}",
                pattern
            )
        })?;

        Ok(Self {
            id: knowledge_id(&namespace, &id),
            namespace,
            title: title.into(),
            content: content.into(),
            pattern_regex,
        })
    }

    fn matches(&self, input: &MatchInput<'_>) -> bool {
        input.is_match(&self.pattern_regex)
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

fn validate_slug(value: &str, path: &Path, index: usize) -> anyhow::Result<()> {
    let has_letter = value.chars().any(|c| c.is_ascii_lowercase());
    let valid_chars = value
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    let valid_edges = !value.starts_with('-') && !value.ends_with('-');
    let valid_runs = !value.contains("--");

    if !(has_letter && valid_chars && valid_edges && valid_runs) {
        anyhow::bail!(
            "inspiration knowledge id is not an ASCII slug in {}[{}]: {}",
            path.display(),
            index,
            value
        );
    }

    Ok(())
}

fn validate_pattern(pattern: &str, path: &Path, index: usize) -> anyhow::Result<()> {
    Regex::new(pattern).with_context(|| {
        format!(
            "inspiration knowledge pattern is not a valid regex in {}[{}]",
            path.display(),
            index
        )
    })?;

    Ok(())
}

fn knowledge_id(namespace: &str, id: &str) -> String {
    if namespace.is_empty() {
        id.to_string()
    } else {
        format!("{}.{}", namespace, id)
    }
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
    let namespace = path_id(root, path)?;
    let mut entries = Vec::with_capacity(items.len());

    for (index, item) in items.into_iter().enumerate() {
        validate_non_empty(&item.id, "id", path, index)?;
        validate_non_empty(&item.title, "title", path, index)?;
        validate_non_empty(&item.pattern, "pattern", path, index)?;
        validate_non_empty(&item.content, "content", path, index)?;
        validate_slug(&item.id, path, index)?;
        validate_pattern(&item.pattern, path, index)?;

        let pattern_regex = Regex::new(&item.pattern).expect("validated regex pattern");

        entries.push(KnowledgeEntry {
            id: knowledge_id(&namespace, &item.id),
            namespace: namespace.clone(),
            title: item.title,
            content: item.content,
            pattern_regex,
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
            .filter(|entry| !state.active_knowledge_ids.contains(&entry.id))
            .filter(|entry| entry.matches(input))
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

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

    fn write_entry(memory_dir: &Path, path: &str, raw: &str) {
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
            r#"[{"id":"member-a","title":"成员A","pattern":"member-name-a","content":"member-a 是一名翻译"}]"#,
        );

        let registry = KnowledgeRegistry::load(memory_dir.clone()).unwrap();
        let input = MatchInput::parse("member-name-a");
        let entries = registry.match_entries(&input, &InspiredState::default());

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "a.b.c.member-a");
        assert_eq!(entries[0].namespace, "a.b.c");
        assert_eq!(entries[0].title, "成员A");
        assert_eq!(entries[0].content, "member-a 是一名翻译");

        std::fs::remove_dir_all(memory_dir).unwrap();
    }

    #[test]
    fn load_rejects_duplicate_actual_id() {
        let memory_dir = temp_memory_dir();
        write_entry(
            &memory_dir,
            "member.lb.json",
            r#"[{"id":"same","title":"LB","pattern":"LB","content":"one"}]"#,
        );
        write_entry(
            &memory_dir,
            "member/lb.json",
            r#"[{"id":"same","title":"LB","pattern":"LB","content":"one"}]"#,
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
            r#"[{"id":"lb","title":"LB","pattern":"","content":"LB"}]"#,
        );

        let err = KnowledgeRegistry::load(memory_dir.clone()).unwrap_err();
        assert!(format!("{:?}", err).contains("field 'pattern' is empty"));

        std::fs::remove_dir_all(memory_dir).unwrap();
    }

    #[test]
    fn load_rejects_non_ascii_slug_id() {
        let memory_dir = temp_memory_dir();
        write_entry(
            &memory_dir,
            "member/lb.json",
            r#"[{"id":"牛牛","title":"牛牛","pattern":"牛牛","content":"翻译"}]"#,
        );

        let err = KnowledgeRegistry::load(memory_dir.clone()).unwrap_err();
        assert!(format!("{:?}", err).contains("id is not an ASCII slug"));

        std::fs::remove_dir_all(memory_dir).unwrap();
    }

    #[test]
    fn match_entry_supports_pipe_separated_patterns() {
        let registry = KnowledgeRegistry::from_entries(vec![
            super::KnowledgeEntry::new(
                "role",
                "huiantianqiong",
                "牛牛",
                "牛牛|灰暗天穹",
                "翻译、校对",
            )
            .unwrap(),
        ]);
        let input = MatchInput::parse("灰暗天穹在吗");
        let entries = registry.match_entries(&input, &InspiredState::default());

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "role.huiantianqiong");
    }

    #[test]
    fn match_entry_uses_regex_pattern() {
        let registry = KnowledgeRegistry::from_entries(vec![
            super::KnowledgeEntry::new("role", "dryice", "Dryice", "(?i)^dryice$", "职位：嵌字")
                .unwrap(),
        ]);
        let input = MatchInput::parse(
            "[channel_id: 1, channel_name: -, sender_id: 2, sender_nickname: DryIce, sender_channel_nickname: -, sender_prks_id: -, sent_at: now]\n在吗",
        );
        let entries = registry.match_entries(&input, &InspiredState::default());

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "role.dryice");
    }

    #[test]
    fn match_entry_supports_case_insensitive_ascii_alias() {
        let registry = KnowledgeRegistry::from_entries(vec![
            super::KnowledgeEntry::new(
                "role",
                "debu-mao-xian-jia",
                "debu冒险家",
                "(?i:debu)(?:冒险家)?|(?i:parody)",
                "职位：翻译 校对，白杨现组长",
            )
            .unwrap(),
        ]);

        for raw in ["Debu", "debu", "DEBU", "Debu冒险家"] {
            let input = MatchInput::parse(raw);
            let entries = registry.match_entries(&input, &InspiredState::default());

            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].id, "role.debu-mao-xian-jia");
        }
    }

    #[test]
    fn load_rejects_invalid_regex_pattern() {
        let memory_dir = temp_memory_dir();
        write_entry(
            &memory_dir,
            "member/lb.json",
            r#"[{"id":"lb","title":"LB","pattern":"(","content":"LB"}]"#,
        );

        let err = KnowledgeRegistry::load(memory_dir.clone()).unwrap_err();
        assert!(format!("{:?}", err).contains("pattern is not a valid regex"));

        std::fs::remove_dir_all(memory_dir).unwrap();
    }
}
