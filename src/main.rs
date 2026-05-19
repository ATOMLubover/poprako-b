use poprako_b_preview::ai;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().expect("[main] Failed to load .env file");

    println!("Hello, world!");
}
