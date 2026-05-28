use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use tokio::sync::mpsc::{self, Receiver, Sender};

use crate::bot::agent::memory_dir;

/// Returns the list of prompt files that the watchdog monitors.
/// Files are assembled in order: scenario → persona → rules → examples.
fn prompt_paths() -> Vec<PathBuf> {
    let dir = memory_dir().join("prompts");
    vec![
        dir.join("scenario.txt"),
        dir.join("persona.txt"),
        dir.join("rules.txt"),
        dir.join("examples.txt"),
    ]
}

/// Assembles the full system prompt from individual prompt files.
/// Order: scenario (context) → persona (identity) → rules (constraints) → examples (demonstration).
/// Examples are placed last so the model sees concrete patterns just before the conversation.
pub fn system_prompt() -> anyhow::Result<String> {
    let dir = memory_dir().join("prompts");

    let scenario = std::fs::read_to_string(dir.join("scenario.txt"))?;
    let persona = std::fs::read_to_string(dir.join("persona.txt"))?;
    let rules = std::fs::read_to_string(dir.join("rules.txt"))?;
    let examples = std::fs::read_to_string(dir.join("examples.txt"))?;

    Ok(format!(
        "{}\n\n{}\n\n{}\n\n{}",
        scenario, persona, rules, examples
    ))
}

/// Spawn a background task that polls the system prompt files every 30 seconds.
/// When a file's `modified` timestamp changes, the task reloads the prompt and
/// sends it through the returned receiver so the main event loop can apply it.
pub fn spawn_refresh_system_promt_task() -> anyhow::Result<Receiver<String>> {
    let (tx, rx) = mpsc::channel(1);
    tokio::spawn(system_prompt_watchdog(tx));

    Ok(rx)
}

async fn system_prompt_watchdog(tx: Sender<String>) {
    let paths = prompt_paths();
    let last_mtimes = Mutex::new(HashMap::<PathBuf, SystemTime>::new());

    // Seed initial timestamps so the first poll does not trigger a reload.
    {
        let mut mtimes = last_mtimes.lock().unwrap();
        for path in &paths {
            if let Ok(meta) = std::fs::metadata(path)
                && let Ok(modified) = meta.modified()
            {
                mtimes.insert(path.clone(), modified);
            }
        }
    }
    loop {
        tokio::time::sleep(Duration::from_secs(30)).await;

        let changed = {
            let mut mtimes = last_mtimes.lock().unwrap();
            let mut any_changed = false;

            for path in &paths {
                let current_mtime = std::fs::metadata(path).ok().and_then(|m| m.modified().ok());

                let Some(current_mtime) = current_mtime else {
                    continue;
                };

                if mtimes.get(path) != Some(&current_mtime) {
                    any_changed = true;
                    mtimes.insert(path.clone(), current_mtime);
                }
            }

            any_changed
        };

        if !changed {
            continue;
        }

        match system_prompt() {
            Ok(content) => {
                tracing::info!("system prompt files changed, reloading...");
                if tx.send(content).await.is_err() {
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
