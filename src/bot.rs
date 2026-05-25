mod agent;
mod handler;
mod message;
mod server;
mod state;

pub async fn run_server() -> anyhow::Result<()> {
    tracing::info!("starting poprako-b bot server");

    server::BotServer::from_env().await?.serve().await
}
