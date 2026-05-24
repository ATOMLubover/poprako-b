use anyhow::Result;
use poprako_b_preview::bot::{BotServer, BotState, Message, Router};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt;
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;

pub async fn handle_group_message(
    _state: BotState,
    msg: Message,
) -> anyhow::Result<Option<Message>> {
    tracing::info!(
        group_id = msg.group_id(),
        user_id = msg.user_id(),
        raw_message = msg.raw_text(),
        "received group message"
    );

    Ok(None)
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    let router = Router::new().on_group_message(handle_group_message);
    BotServer::from_env().await?.serve(router).await
}
