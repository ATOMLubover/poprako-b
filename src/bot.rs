mod agent;
mod handler;
mod keepalive;
mod message;
mod scheduled_task;
mod server;
mod state;
mod value_object;

pub async fn run_server() -> anyhow::Result<()> {
    tracing::info!("starting poprako-b bot server");

    server::BotServer::from_env().await?.serve().await
}
