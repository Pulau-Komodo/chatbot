#![allow(clippy::get_first)]

use std::fs;

use chatgpt::Chatgpt;
use config::{Config, CustomApiKeys};
use database::init_database;
use discord_client::DiscordEventHandler;
use serenity::{http::Http, prelude::GatewayIntents};

mod allowances;
mod chatgpt;
mod config;
mod conversations;
mod database;
mod discord_client;
mod one_off_response;
mod response_styles;
mod user_settings;
mod util;

#[tokio::main]
async fn main() {
	let db_pool = init_database("./data/db.db").await;

	let discord_token = fs::read_to_string("./token.txt").expect("Could not read token file");

	let chatgpt_api_key =
		fs::read_to_string("./gpt_api_key.txt").expect("Could not read GPT API key file");

	let config = Config::from_file("./config.toml");
	let custom_api_keys = CustomApiKeys::from_file("./custom_api_keys.toml");

	let chatgpt = Chatgpt::new(chatgpt_api_key, None, config, custom_api_keys).unwrap();

	let my_id = Http::new(&discord_token)
		.get_current_user()
		.await
		.unwrap()
		.id;

	let handler = DiscordEventHandler::new(db_pool, chatgpt, my_id);
	let mut client = serenity::Client::builder(
		&discord_token,
		GatewayIntents::GUILDS | GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT,
	)
	.event_handler(handler)
	.await
	.expect("Error creating Discord client");

	if let Err(why) = client.start().await {
		eprintln!("Error starting client: {:?}", why);
	}
}
