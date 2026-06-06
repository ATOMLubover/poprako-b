use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use tokio::sync::mpsc::{self, Receiver, Sender};

use crate::bot::agent::memory_dir;

/// Returns the list of prompt files that the watchdog monitors.
/// Files are assembled in order: persona → directory → directory sections → examples.
fn prompt_paths() -> Vec<PathBuf> {
    let dir = memory_dir().join("prompts");
    vec![
        dir.join("persona.txt"),
        dir.join("directory.txt"),
        dir.join("scene.txt"),
        dir.join("input-format.txt"),
        dir.join("injected-context.txt"),
        dir.join("knowledge-tools.txt"),
        dir.join("response-style.txt"),
        dir.join("safety.txt"),
        dir.join("examples.txt"),
    ]
}

/// Assembles the full system prompt from individual prompt files.
/// Order: persona (identity) → directory → directory sections → examples (demonstration).
/// Examples are placed last so the model sees concrete patterns just before the conversation.
pub fn system_prompt() -> anyhow::Result<String> {
    prompt_paths()
        .into_iter()
        .map(std::fs::read_to_string)
        .collect::<Result<Vec<_>, _>>()
        .map(|parts| parts.join("\n\n"))
        .map_err(Into::into)
}

async fn system_prompt_watchdog(send: Sender<String>) {
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
