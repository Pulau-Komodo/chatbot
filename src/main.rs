use std::fs;

use chatgpt::Chatgpt;
use discord_client::DiscordEventHandler;
use serenity::prelude::GatewayIntents;
use sqlx::sqlite::SqlitePoolOptions;

mod allowances;
mod chatgpt;
mod conversations;
mod discord_client;
mod util;

#[tokio::main]
async fn main() {
	let token = fs::read_to_string("./token.txt").expect("Could not read token file");

	let db_pool = SqlitePoolOptions::new()
		.max_connections(4)
		.connect("./data/db.db")
		.await
		.unwrap();

	let chatgpt_api_key =
		fs::read_to_string("./gpt_api_key.txt").expect("Could not read GPT API key file");
	let chatgpt = Chatgpt::new(chatgpt_api_key, None).unwrap();

	let handler = DiscordEventHandler::new(db_pool, chatgpt);
	let mut client = serenity::Client::builder(
		&token,
		GatewayIntents::GUILDS | GatewayIntents::GUILD_MESSAGES,
	)
	.event_handler(handler)
	.await
	.expect("Error creating Discord client");

	if let Err(why) = client.start().await {
		eprintln!("Error starting client: {:?}", why);
	}
}
