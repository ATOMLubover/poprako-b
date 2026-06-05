use rand::Rng;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::sync::mpsc;
use tokio::time::sleep;

#[derive(Debug)]
pub struct KeepaliveTrigger {
    pub texts: Vec<String>,
}

/// Return the current hour in local time (assumes UTC+8 / China Standard Time).
fn local_hour() -> u64 {
    let since_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    ((since_epoch % 86_400) / 3_600 + 8) % 24
}

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

pub fn watch_keepalive() -> anyhow::Result<mpsc::Receiver<KeepaliveTrigger>> {
    let (send, recv) = mpsc::channel(1);
    tokio::spawn(keepalive_watchdog(send));

    Ok(recv)
}

async fn keepalive_watchdog(send: mpsc::Sender<KeepaliveTrigger>) {
    loop {
        let interval_secs = rand::thread_rng().gen_range(120..=300);
        sleep(Duration::from_secs(interval_secs)).await;

        let hour = local_hour();
        if (1..9).contains(&hour) {
            tracing::debug!("keepalive skipped: hour={} in quiet period (1-9)", hour);
            continue;
        }

        let count = rand::thread_rng().gen_range(1..=3);
        let texts = (0..count)
            .map(|_| {
                let idx = rand::thread_rng().gen_range(0..KEEPALIVE_MESSAGES.len());
                KEEPALIVE_MESSAGES[idx].to_string()
            })
            .collect();

        if send.send(KeepaliveTrigger { texts }).await.is_err() {
            tracing::warn!("keepalive receiver dropped, watchdog exiting");
            return;
        }
    }
}
