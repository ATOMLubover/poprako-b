mod agent;
mod handler;
mod keepalive;
mod message;
mod scheduled_task;
mod server;
mod state;

use crate::bot::agent::BotAgent;
use crate::bot::agent::watch_system_prompt;
use crate::bot::handler::handle_channel_message;
use crate::bot::handler::handle_keepalive_trigger;
use crate::bot::handler::handle_scheduled_spam_trigger;
use crate::bot::handler::handle_system_prompt_refresh;
use crate::bot::keepalive::watch_keepalive;
use crate::bot::scheduled_task::watch_scheduled_spam;
use crate::bot::server::BotServer;
use crate::bot::server::config::BotServerConfig;
use crate::bot::state::BotState;

pub async fn run_server() -> anyhow::Result<()> {
    tracing::info!("starting poprako-b bot server");

    let config = BotServerConfig::from_env()?;
    let agent = BotAgent::new().await?;

    BotServer::new(BotState::new(agent, config.self_id))
        .with_onebot_from_env()?
        .on_channel_message(handle_channel_message)
        .on_notification(watch_system_prompt, handle_system_prompt_refresh)
        .on_notification(watch_scheduled_spam, handle_scheduled_spam_trigger)
        .on_notification(watch_keepalive, handle_keepalive_trigger)
        .serve()
        .await
}
