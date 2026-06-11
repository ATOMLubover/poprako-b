use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use serde::Deserialize;
use tokio::sync::mpsc::{self, Receiver, Sender};

use crate::ai::resolver::message::{SystemItem, SystemMessage};
use crate::bot::agent::memory_dir;

// ---------------------------------------------------------------------------
// Manifest types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct SectionEntry {
    id: String,
    title: String,
    path: String,
    #[serde(default = "default_enabled")]
    enabled: bool,
}

fn default_enabled() -> bool {
    true
}

#[derive(Debug, Deserialize)]
struct Manifest {
    #[serde(default)]
    embedded: Vec<SectionEntry>,
    #[serde(default)]
    #[allow(dead_code)] // TODO: reserved for v2 plugin sections
    plugins: Vec<SectionEntry>,
}

// ---------------------------------------------------------------------------
// Default manifest (hard-coded fallback when system.yaml is missing)
// ---------------------------------------------------------------------------

fn default_manifest_entries() -> Vec<SectionEntry> {
    vec![
        SectionEntry {
            id: "persona".into(),
            title: "角色身份".into(),
            path: "persona.txt".into(),
            enabled: true,
        },
        SectionEntry {
            id: "directory".into(),
            title: "提示目录".into(),
            path: "directory.txt".into(),
            enabled: true,
        },
        SectionEntry {
            id: "scene".into(),
            title: "场景".into(),
            path: "scene.txt".into(),
            enabled: true,
        },
        SectionEntry {
            id: "input-format".into(),
            title: "用户消息格式".into(),
            path: "input-format.txt".into(),
            enabled: true,
        },
        SectionEntry {
            id: "injected-context".into(),
            title: "注入上下文".into(),
            path: "injected-context.txt".into(),
            enabled: true,
        },
        SectionEntry {
            id: "knowledge-tools".into(),
            title: "知识与工具".into(),
            path: "knowledge-tools.txt".into(),
            enabled: true,
        },
        SectionEntry {
            id: "response-style".into(),
            title: "发言格式".into(),
            path: "response-style.txt".into(),
            enabled: true,
        },
        SectionEntry {
            id: "safety".into(),
            title: "账号安全".into(),
            path: "safety.txt".into(),
            enabled: true,
        },
        SectionEntry {
            id: "examples".into(),
            title: "对话示例".into(),
            path: "examples.txt".into(),
            enabled: true,
        },
    ]
}

// ---------------------------------------------------------------------------
// Directory helpers
// ---------------------------------------------------------------------------

fn prompts_dir() -> PathBuf {
    memory_dir().join("prompts")
}

// FIXME: bad naming.
fn manifest_path_in(dir: &Path) -> PathBuf {
    dir.join("system.yaml")
}

// ---------------------------------------------------------------------------
// Manifest loading (path-parameterised for testability)
// ---------------------------------------------------------------------------

fn load_manifest(dir: &Path) -> anyhow::Result<Manifest> {
    let path = manifest_path_in(dir);
    match std::fs::read_to_string(&path) {
        Ok(yaml) => Ok(serde_yaml::from_str(&yaml)?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            tracing::warn!(
                "system.yaml not found at {}, using built-in default manifest",
                path.display()
            );
            Ok(Manifest {
                embedded: default_manifest_entries(),
                plugins: Vec::new(),
            })
        }
        Err(e) => Err(e.into()),
    }
}

// ---------------------------------------------------------------------------
// Path list for the watchdog
// ---------------------------------------------------------------------------

/// Collect every filesystem path the watchdog should monitor:
/// the manifest itself plus every enabled embedded text file.
fn watched_paths_in(dir: &Path, manifest: &Manifest) -> Vec<PathBuf> {
    let mut paths = vec![manifest_path_in(dir)];
    for entry in &manifest.embedded {
        if entry.enabled {
            paths.push(dir.join(&entry.path));
        }
    }

    paths
}

fn watched_paths(manifest: &Manifest) -> Vec<PathBuf> {
    watched_paths_in(&prompts_dir(), manifest)
}

// ---------------------------------------------------------------------------
// System message assembly
// ---------------------------------------------------------------------------

/// Load enabled embedded sections from the manifest and build a `SystemMessage`.
/// Reads manifest and text files from `dir`.
fn system_message(dir: &Path) -> anyhow::Result<SystemMessage> {
    let manifest = load_manifest(dir)?;

    let mut embedded = Vec::with_capacity(manifest.embedded.len());
    for entry in &manifest.embedded {
        if !entry.enabled {
            continue;
        }
        let file_path = dir.join(&entry.path);
        let content = std::fs::read_to_string(&file_path).map_err(|e| {
            anyhow::anyhow!(
                "failed to read prompt file '{}' for section '{}': {e}",
                file_path.display(),
                entry.id,
            )
        })?;

        // Validate early: no </section> in raw content.
        if content.contains("</section>") {
            anyhow::bail!(
                "prompt file '{}' (section '{}') contains forbidden </section> tag",
                file_path.display(),
                entry.id,
            );
        }

        embedded.push(SystemItem {
            id: entry.id.clone(),
            title: entry.title.clone(),
            content,
        });
    }

    Ok(SystemMessage::new(embedded, Vec::new()))
}

/// Legacy entrypoint: assemble the full system prompt as a rendered XML string.
/// Kept for backward compatibility with callers that expect a `String`.
pub fn system_prompt() -> anyhow::Result<String> {
    Ok(system_message(&prompts_dir())?.render())
}

// ---------------------------------------------------------------------------
// Watchdog
// ---------------------------------------------------------------------------

async fn system_prompt_watchdog(send: Sender<String>) {
    // Prime with the current manifest for watched-paths tracking.
    let mut current_paths = match load_manifest(&prompts_dir()) {
        Ok(m) => watched_paths(&m),
        Err(e) => {
            tracing::error!("watchdog failed to load manifest on start: {e}");
            return;
        }
    };
    let last_mtimes = Mutex::new(HashMap::<PathBuf, SystemTime>::new());

    // Seed initial timestamps so the first poll does not trigger a reload.
    {
        let mut mtimes = last_mtimes.lock().unwrap();
        for path in &current_paths {
            if let Ok(meta) = std::fs::metadata(path)
                && let Ok(modified) = meta.modified()
            {
                mtimes.insert(path.clone(), modified);
            }
        }
    }

    loop {
        tokio::time::sleep(Duration::from_secs(30)).await;

        // Reload the manifest every cycle so we pick up new/removed sections.
        let manifest = match load_manifest(&prompts_dir()) {
            Ok(m) => m,
            Err(e) => {
                tracing::error!("watchdog failed to reload manifest: {e}");
                continue;
            }
        };
        current_paths = watched_paths(&manifest);

        let changed = {
            let mut mtimes = last_mtimes.lock().unwrap();
            let mut any_changed = false;

            for path in &current_paths {
                let current_mtime = std::fs::metadata(path).ok().and_then(|m| m.modified().ok());

                let Some(current_mtime) = current_mtime else {
                    continue;
                };

                if mtimes.get(path) != Some(&current_mtime) {
                    any_changed = true;
                    mtimes.insert(path.clone(), current_mtime);
                }
            }

            // Prune entries for files no longer in the manifest.
            mtimes.retain(|p, _| current_paths.contains(p));

            any_changed
        };

        if !changed {
            continue;
        }

        match system_prompt() {
            Ok(content) => {
                tracing::info!("system prompt files changed, reloading...");
                if send.send(content).await.is_err() {
                    tracing::warn!("prompt refresh receiver dropped, watchdog exiting");
                    break;
                }
            }
            Err(e) => {
                tracing::error!("failed to reload system prompt: {e}");
            }
        }
    }
}

/// Spawn a background task that polls the system prompt files every 30 seconds.
/// When a file's `modified` timestamp changes, the task reloads the prompt and
/// sends it through the returned receiver so the main event loop can apply it.
pub fn watch_system_prompt() -> anyhow::Result<Receiver<String>> {
    let (send, recv) = mpsc::channel(1);
    tokio::spawn(system_prompt_watchdog(send));

    Ok(recv)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a temporary `prompts/` directory with the given files
    /// and a `system.yaml` manifest string.
    ///
    /// Returns the `TempDir` (keep-alive) and the `prompts` subdirectory path.
    fn setup(files: &[(&str, &str)], manifest_yaml: &str) -> (tempfile::TempDir, PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let prompts = tmp.path().join("prompts");
        std::fs::create_dir_all(&prompts).unwrap();
        for (name, content) in files {
            std::fs::write(prompts.join(name), content).unwrap();
        }
        std::fs::write(prompts.join("system.yaml"), manifest_yaml).unwrap();
        (tmp, prompts)
    }

    // -- system_message_from (path-parameterised) --

    #[test]
    fn test_disabled_section_not_in_output() {
        let (_tmp, prompts) = setup(
            &[
                ("enabled.txt", "enabled content"),
                ("disabled.txt", "disabled content"),
            ],
            r#"
embedded:
  - id: enabled
    title: "Enabled"
    path: enabled.txt
    enabled: true
  - id: disabled
    title: "Disabled"
    path: disabled.txt
    enabled: false
plugins: []
"#,
        );

        let sm = system_message(&prompts).unwrap();
        let rendered = sm.render();

        assert!(rendered.contains("enabled content"));
        assert!(!rendered.contains("disabled content"));
        assert!(!rendered.contains("Disabled"));
    }

    #[test]
    fn test_missing_text_file_returns_error() {
        let (_tmp, prompts) = setup(
            &[],
            r#"
embedded:
  - id: missing
    title: "Missing"
    path: does-not-exist.txt
    enabled: true
plugins: []
"#,
        );

        let result = system_message(&prompts);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("does-not-exist.txt"),
            "error should mention missing file: {err}"
        );
    }

    #[test]
    fn test_forbidden_section_close_tag_returns_error() {
        let (_tmp, prompts) = setup(
            &[("bad.txt", "content with </section> inside")],
            r#"
embedded:
  - id: bad
    title: "Bad"
    path: bad.txt
    enabled: true
plugins: []
"#,
        );

        let result = system_message(&prompts);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("</section>"),
            "error should mention forbidden tag: {err}"
        );
    }

    #[test]
    fn test_system_prompt_includes_existing_content() {
        let (_tmp, prompts) = setup(
            &[
                ("persona.txt", "I am a bot."),
                ("scene.txt", "You are in a chat room."),
            ],
            r#"
embedded:
  - id: persona
    title: "Persona"
    path: persona.txt
    enabled: true
  - id: scene
    title: "Scene"
    path: scene.txt
    enabled: true
plugins: []
"#,
        );

        let sm = system_message(&prompts).unwrap();
        let prompt = sm.render();

        assert!(prompt.contains("I am a bot."));
        assert!(prompt.contains("You are in a chat room."));
        assert!(prompt.starts_with("<system>"));
        assert!(prompt.ends_with("</system>"));
    }

    #[test]
    fn test_embedded_before_plugins_in_system_prompt() {
        let (_tmp, prompts) = setup(
            &[("e.txt", "embedded")],
            r#"
embedded:
  - id: e
    title: "Emb"
    path: e.txt
    enabled: true
plugins: []
"#,
        );

        let sm = system_message(&prompts).unwrap();
        let prompt = sm.render();
        let _emb_pos = prompt.find("<embedded>").unwrap();
        assert!(prompt.contains("<embedded>"));
        // No <plugins> block expected since v1 has no plugin sections.
    }

    #[test]
    fn test_manifest_missing_fallback() {
        let tmp = tempfile::tempdir().unwrap();
        let prompts = tmp.path().join("prompts");
        std::fs::create_dir_all(&prompts).unwrap();

        // Write default text files but NO system.yaml.
        for entry in default_manifest_entries() {
            std::fs::write(prompts.join(&entry.path), format!("{} content", entry.id)).unwrap();
        }

        let sm = system_message(&prompts).expect("should fall back to default manifest");
        let rendered = sm.render();

        assert!(rendered.contains("<section id=\"persona\""));
        assert!(rendered.contains("persona content"));
        assert!(rendered.contains("<section id=\"examples\""));
        assert!(rendered.contains("examples content"));
    }

    #[test]
    fn test_watched_paths_includes_manifest_and_enabled_files() {
        let tmp = tempfile::tempdir().unwrap();
        let prompts = tmp.path().join("prompts");
        std::fs::create_dir_all(&prompts).unwrap();

        let manifest = Manifest {
            embedded: vec![
                SectionEntry {
                    id: "a".into(),
                    title: "A".into(),
                    path: "a.txt".into(),
                    enabled: true,
                },
                SectionEntry {
                    id: "b".into(),
                    title: "B".into(),
                    path: "b.txt".into(),
                    enabled: false,
                },
            ],
            plugins: vec![],
        };

        let paths = watched_paths_in(&prompts, &manifest);

        assert!(paths.contains(&prompts.join("system.yaml")));
        assert!(paths.contains(&prompts.join("a.txt")));
        assert!(!paths.contains(&prompts.join("b.txt"))); // disabled
    }
}
