use itertools::Itertools;
use serenity::{async_trait, model::prelude::*, prelude::*};
use sqlx::{Pool, Sqlite};

use crate::{allowances, chatgpt::Chatgpt, user_settings};

/// If there is a mention on either end of the string, removes it and trims. Removes only one mention.
fn strip_mention(text: String, mentions: &[String]) -> String {
	let new_text = [str::strip_prefix, str::strip_suffix]
		.into_iter()
		.cartesian_product(mentions)
		.find_map(|(strip, mention)| strip(&text, mention))
		.map(str::trim);
	new_text.map(String::from).unwrap_or(text)
}

async fn get_referenced_contents(
	http: &std::sync::Arc<serenity::http::Http>,
	mut referenced: Box<Message>,
) -> Option<String> {
	if !referenced.content.is_empty() {
		return Some(std::mem::take(&mut referenced.content));
	}
	let Ok(mut referenced) = http.get_message(referenced.channel_id, referenced.id).await else {
		return None;
	};
	if !referenced.content.is_empty() {
		Some(std::mem::take(&mut referenced.content))
	} else {
		None
	}
}

pub struct DiscordEventHandler {
	database: Pool<Sqlite>,
	chatgpt: Chatgpt,
	mentions: [String; 2],
}

impl DiscordEventHandler {
	pub fn new(database: Pool<Sqlite>, chatgpt: Chatgpt, own_user_id: UserId) -> Self {
		let mention = format!("<@{}>", own_user_id.get());
		let mention_nick = format!("<@!{}>", own_user_id.get());
		let mentions = [mention, mention_nick];
		Self {
			database,
			chatgpt,
			mentions,
		}
	}
	/// The message looks like something to start or continue a conversation with.
	async fn handle_conversation_message(&self, context: Context, mut message: Message) {
		let content = std::mem::take(&mut message.content);
		if let Some(referenced) = std::mem::take(&mut message.referenced_message) {
			if referenced.author.id == context.cache.current_user().id {
				// A message replying to the bot's own message
				self.chatgpt
					.query(
						&self.database,
						context,
						content,
						message,
						Some(referenced.id),
					)
					.await;
			} else if let Some(referenced_contents) =
				get_referenced_contents(&context.http, referenced).await
			{
				let mut text = strip_mention(content, &self.mentions);
				if text.is_empty() {
					// A message replying to something, but containing nothing but a mention to the bot
					let referenced_contents = strip_mention(referenced_contents, &self.mentions); // Stripping mentions so replies can be used to repeat queries, possibly with different settings
					if referenced_contents.is_empty() {
						return; // Referenced message had only a mention, makes no sense, ignore
					}
					self.chatgpt
						.query(&self.database, context, referenced_contents, message, None)
						.await;
				} else {
					// A message replying to something, and containing its own text as well
					use std::fmt::Write;
					write!(text, " \"{referenced_contents}\"").unwrap();
					self.chatgpt
						.query(&self.database, context, text, message, None)
						.await;
				}
			} else {
				// It has a referenced message, but the bot couldn't get it
				println!(
					"Could not get message referenced by message {}",
					message.id.get()
				);
			}
		} else {
			// A message not replying to anything, and pinging the bot
			let old_len = content.len();
			let text = strip_mention(content, &self.mentions);
			if old_len == text.len() {
				// Pinged the bot but had no mention at either end, so don't take it as being addressed.
				return;
			}
			self.chatgpt
				.query(&self.database, context, text, message, None)
				.await;
		}
	}
}

#[async_trait]
impl EventHandler for DiscordEventHandler {
	async fn message(&self, context: Context, message: Message) {
		let own_id = context.cache.current_user().id;
		if !message.is_own(&context.cache)
			&& message.mentions_user_id(own_id)
			&& !message.content.is_empty()
		{
			self.handle_conversation_message(context, message).await;
		}
	}

	async fn interaction_create(&self, context: Context, interaction: Interaction) {
		if let Interaction::Command(interaction) = interaction {
			match interaction.data.name.as_str() {
				"allowance" => {
					allowances::command_check(context, interaction, &self.database)
						.await
						.unwrap();
				}
				"spent" => {
					allowances::command_expenditure(context, interaction, &self.database)
						.await
						.unwrap();
				}
				"gpt4" => user_settings::command_set_gpt4(context, interaction, &self.database)
					.await
					.unwrap(),
				"personality" => {
					user_settings::command_set_system_message(context, interaction, &self.database)
						.await
						.unwrap()
				}
				_ => (),
			};
		}
	}

	async fn ready(&self, context: Context, _ready: Ready) {
		println!("Ready");
		let arg = std::env::args().nth(1);
		if let Some(arg) = arg {
			if &arg == "register" {
				for guild in context.cache.guilds() {
					let commands = vec![
						allowances::register(),
						allowances::register_check_expenditure(),
						user_settings::register_set_gpt4(),
						user_settings::register_set_system_message(),
					];
					let commands = guild.set_commands(&context.http, commands).await.unwrap();
					let command_names = commands.into_iter().map(|command| command.name).join(", ");
					println!(
						"I now have the following guild slash commands in guild {}: {}",
						guild.get(),
						command_names
					);
				}
			}
		}
	}
}
