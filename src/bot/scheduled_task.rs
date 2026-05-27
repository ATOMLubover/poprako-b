use std::env;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use chrono::FixedOffset;
use cron::Schedule;
use onebot_v11::MessageSegment;
use onebot_v11::api::payload::{ApiPayload, SendGroupMsg};
use onebot_v11::connect::ws_reverse::ReverseWsConnect;
use tokio::time::sleep;

/// Fixed text sent at midnight to spam groups.
const SPAM_TEXT: &str = "12 点了！.jpg";

/// Parse `SPAM_GROUPS` env var (comma-separated group IDs).
fn parse_spam_groups() -> Vec<i64> {
    env::var("SPAM_GROUPS")
        .unwrap_or_default()
        .split(',')
        .filter_map(|s| {
            let s = s.trim();
            if s.is_empty() {
                None
            } else {
                s.parse::<i64>().ok()
            }
        })
        .collect()
}

/// Spawn a task that sends `SPAM_TEXT` to `SPAM_GROUPS` at midnight (00:00 UTC+8)
/// every day.
pub fn spawn_spam_task(conn: Arc<ReverseWsConnect>) {
    let schedule = match Schedule::from_str("0 0 0 * * * *") {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("failed to parse cron expression: {e}");
            return;
        }
    };

    let timezone = match FixedOffset::east_opt(8 * 3600) {
        Some(tz) => tz,
        None => {
            tracing::error!("failed to create UTC+8 timezone");
            return;
        }
    };

    let groups = parse_spam_groups();
    if groups.is_empty() {
        tracing::warn!("SPAM_GROUPS is empty, scheduled task will do nothing");
    } else {
        tracing::info!("scheduled task will send to {} group(s)", groups.len());
    }

    tokio::spawn(async move {
        loop {
            let next = match schedule.upcoming(timezone).next() {
                Some(t) => t,
                None => {
                    tracing::error!("cron schedule has no upcoming times");
                    return;
                }
            };

            let now = chrono::Utc::now().with_timezone(&timezone);
            let wait = match (next - now).to_std() {
                Ok(d) if d > Duration::from_secs(0) => d,
                _ => {
                    tracing::warn!(
                        "scheduled task: next run {next} is in the past, retrying in 60s"
                    );
                    sleep(Duration::from_secs(60)).await;
                    continue;
                }
            };

            tracing::info!("scheduled task: next run at {next} (in {wait:?})");
            sleep(wait).await;

            if groups.is_empty() {
                tracing::error!("scheduled task: SPAM_GROUPS is empty, skipping");
                return;
            }

            tracing::info!("scheduled task: sending to {} group(s)", groups.len());

            for (i, &group_id) in groups.iter().enumerate() {
                let payload = ApiPayload::SendGroupMsg(SendGroupMsg {
                    group_id,
                    message: vec![MessageSegment::text(SPAM_TEXT)],
                    auto_escape: false,
                });

                match conn.clone().call_api(payload).await {
                    Ok(_) => tracing::info!(
                        "scheduled {}/{} sent to group {group_id}",
                        i + 1,
                        groups.len()
                    ),
                    Err(e) => tracing::warn!(
                        "scheduled {}/{} failed for group {group_id}: {e}",
                        i + 1,
                        groups.len()
                    ),
                }

                if i + 1 < groups.len() {
                    sleep(Duration::from_secs(2)).await;
                }
            }
        }
    });
}
