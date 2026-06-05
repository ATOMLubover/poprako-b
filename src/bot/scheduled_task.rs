use std::env;
use std::str::FromStr;
use std::time::Duration;

use chrono::FixedOffset;
use cron::Schedule;
use tokio::sync::mpsc;
use tokio::time::sleep;

/// Path to the base64-encoded image sent at midnight and on boot.
const SPAM_IMAGE_PATH: &str = "assets/12oclock.txt";

#[derive(Debug)]
pub enum ScheduledSpamTrigger {
    Boot {
        image_base64: String,
    },
    Midnight {
        channel_ids: Vec<String>,
        image_base64: String,
    },
}

pub fn watch_scheduled_spam() -> anyhow::Result<mpsc::Receiver<ScheduledSpamTrigger>> {
    let (send, recv) = mpsc::channel(1);
    tokio::spawn(scheduled_spam_watchdog(send));

    Ok(recv)
}

fn parse_spam_channels() -> Vec<String> {
    env::var("SPAM_CHANNELS")
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
}

async fn scheduled_spam_watchdog(send: mpsc::Sender<ScheduledSpamTrigger>) {
    let schedule = match Schedule::from_str("0 0 0 * * * *") {
        Ok(schedule) => schedule,
        Err(e) => {
            tracing::error!("failed to parse cron expression: {}", e);
            return;
        }
    };

    let timezone = match FixedOffset::east_opt(8 * 3600) {
        Some(timezone) => timezone,
        None => {
            tracing::error!("failed to create UTC+8 timezone");
            return;
        }
    };

    let channel_ids = parse_spam_channels();
    if channel_ids.is_empty() {
        tracing::warn!("SPAM_CHANNELS is empty, scheduled spam will only send boot message");
    } else {
        tracing::info!(
            "scheduled spam will send to {} channel(s)",
            channel_ids.len()
        );
    }

    let image_base64 = match std::fs::read_to_string(SPAM_IMAGE_PATH) {
        Ok(content) => content,
        Err(e) => {
            tracing::warn!("scheduled spam failed to read '{}': {}", SPAM_IMAGE_PATH, e);
            return;
        }
    };

    if send
        .send(ScheduledSpamTrigger::Boot {
            image_base64: image_base64.clone(),
        })
        .await
        .is_err()
    {
        tracing::warn!("scheduled spam receiver dropped before boot message");
        return;
    }

    loop {
        let next = match schedule.upcoming(timezone).next() {
            Some(next) => next,
            None => {
                tracing::error!("cron schedule has no upcoming times");
                return;
            }
        };

        let now = chrono::Utc::now().with_timezone(&timezone);
        let wait = match (next - now).to_std() {
            Ok(duration) if duration > Duration::from_secs(0) => duration,
            _ => {
                tracing::warn!(
                    "scheduled spam next run {} is in the past, retrying in 60s",
                    next
                );
                sleep(Duration::from_secs(60)).await;
                continue;
            }
        };

        tracing::info!("scheduled spam next run at {} (in {:?})", next, wait);
        sleep(wait).await;

        if channel_ids.is_empty() {
            tracing::warn!("SPAM_CHANNELS is empty, skipping scheduled spam");
            continue;
        }

        if send
            .send(ScheduledSpamTrigger::Midnight {
                channel_ids: channel_ids.clone(),
                image_base64: image_base64.clone(),
            })
            .await
            .is_err()
        {
            tracing::warn!("scheduled spam receiver dropped, watchdog exiting");
            return;
        }
    }
}
