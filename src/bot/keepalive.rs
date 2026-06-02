use rand::Rng;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use onebot_v11::MessageSegment;
use onebot_v11::api::payload::{ApiPayload, SendPrivateMsg};
use onebot_v11::connect::ws_reverse::ReverseWsConnect;
use tokio::time::sleep;

/// Return the current hour in local time (assumes UTC+8 / China Standard Time).
fn local_hour() -> u64 {
    let since_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // UTC+8 → add 8 hours, wrap at 24.
    ((since_epoch % 86_400) / 3_600 + 8) % 24
}

/// Pool of keep-alive messages sent to self during quiet periods.
const KEEPALIVE_MESSAGES: &[&str] = &[
    "白杨子心跳检测 ✓",
    "白杨子运行中，一切正常",
    "系统自检完成，状态良好",
    "白杨子在线，等待召唤",
    "心跳包 —— 我还活着哦",
    "自检通过，内存正常，连接稳定",
    "白杨子待命中...",
    "一切安好，无需担心",
    "今天也是元气满满的一天",
    "白杨子已就绪，随时待命",
    "系统运行流畅，无异常",
    "在线时长 +1",
    "悄悄冒个泡",
    "风平浪静，岁月静好",
    "守护汉化组中...",
    "白杨子巡逻中，一切安全",
    "后台默默工作中",
    "连接稳定，信号满格",
    "今天没有人召唤我呢",
    "组员们都在努力吧",
    "白杨子的日常：等待被 @",
    "保持在线，随时响应",
];

/// Spawn an independent keep-alive task so incoming events (including
/// non-message heartbeat/meta events from NapCat) never cancel the timer.
pub fn spawn_keepalive_task(conn: Arc<ReverseWsConnect>, self_id: i64) {
    tokio::spawn(async move {
        loop {
            let interval_secs = rand::thread_rng().gen_range(120..=300);
            sleep(Duration::from_secs(interval_secs)).await;

            let hour = local_hour();
            if (1..9).contains(&hour) {
                tracing::debug!("keep-alive skipped: hour={hour} in quiet period (1-9)");
                continue;
            }

            let count = rand::thread_rng().gen_range(1..=3);
            tracing::info!("keep-alive: sending {count} message(s) to self");

            for i in 0..count {
                let idx = rand::thread_rng().gen_range(0..KEEPALIVE_MESSAGES.len());
                let text = KEEPALIVE_MESSAGES[idx];

                let payload = ApiPayload::SendPrivateMsg(SendPrivateMsg {
                    user_id: self_id,
                    message: vec![MessageSegment::text(text)],
                    auto_escape: false,
                });

                match conn.clone().call_api(payload).await {
                    Ok(_) => {
                        tracing::info!("keep-alive {}/{} sent: {text}", i + 1, count);
                    }
                    Err(e) => {
                        tracing::warn!("keep-alive message {}/{} failed: {e}", i + 1, count);
                    }
                }

                // Small gap between consecutive messages to avoid burst rate-limit.
                if i + 1 < count {
                    sleep(Duration::from_secs(2)).await;
                }
            }
        }
    });
}
