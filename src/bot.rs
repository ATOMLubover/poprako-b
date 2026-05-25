mod agent;
mod handler;
mod message;
mod router;
mod server;
mod state;

use handler::handle_group_message;
use router::Router;
use server::BotServer;

fn init_router() -> Router {
    Router::new().on_group_message(handle_group_message)
}

pub async fn run_server() -> anyhow::Result<()> {
    tracing::info!("starting poprako-b bot server");

    let router = init_router();

    BotServer::from_env().await?.serve(router).await
}
