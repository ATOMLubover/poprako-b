mod agent;
mod app;
mod event;
mod keepalive;
mod message;
mod policy;
mod scheduled_task;
mod server;
mod state;

use crate::bot::agent::{BotAgent, watch_system_prompt};
use crate::bot::app::BotApp;
use crate::bot::event::BotEvent;
use crate::bot::keepalive::watch_keepalive;
use crate::bot::scheduled_task::watch_scheduled_spam;
use crate::bot::server::BotServer;
use crate::bot::server::config::BotServerConfig;
use crate::bot::state::BotState;

pub async fn run_bot() -> anyhow::Result<()> {
    tracing::info!("starting poprako-b bot server");

    let (review_event_send, review_event_recv) = tokio::sync::mpsc::channel(32);
    let agent = BotAgent::new(review_event_send).await?;

    let config = BotServerConfig::from_env()?;
    let state = BotState::new(agent, config.self_id);

    let app = BotApp::new(state);

    BotServer::new(app)
        .with_onebot_from_env()?
        .on_event_source(watch_system_prompt, BotEvent::SystemPromptRefresh)
        .on_event_source(watch_scheduled_spam, BotEvent::ScheduledSpam)
        .on_event_source(watch_keepalive, BotEvent::Keepalive)
        .on_event_source(move || Ok(review_event_recv), BotEvent::ReviewFollowup)
        .serve()
        .await
}
