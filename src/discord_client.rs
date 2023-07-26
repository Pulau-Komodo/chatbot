use itertools::Itertools;
use serenity::{async_trait, model::prelude::*, prelude::*};
use sqlx::{Pool, Sqlite};

use crate::{allowances, chatgpt::Chatgpt};

/// If there is a mention on either end of the message, this returns that message with the mention removed, and trimmed.
fn strip_mention(text: &str, user_id: UserId) -> Option<&str> {
	let mention = format!("<@{}>", user_id.as_u64());
	let mention_exclamation_mark = format!("<@!{}>", user_id.as_u64());
	text.strip_prefix(&mention)
		.or_else(|| text.strip_prefix(&mention_exclamation_mark))
		.or_else(|| text.strip_suffix(&mention))
		.or_else(|| text.strip_suffix(&mention_exclamation_mark))
		.map(str::trim)
}

pub struct DiscordEventHandler {
	database: Pool<Sqlite>,
	chatgpt: Chatgpt,
}

impl DiscordEventHandler {
	pub fn new(database: Pool<Sqlite>, chatgpt: Chatgpt) -> Self {
		Self { database, chatgpt }
	}
}

#[async_trait]
impl EventHandler for DiscordEventHandler {
	async fn message(&self, context: Context, message: Message) {
		let own_id = context.cache.current_user_id();
		if !message.is_own(&context.cache)
			&& message.mentions_user_id(own_id)
			&& !message.content.is_empty()
		{
			if let Some(text) = strip_mention(&message.content, own_id) {
				let text = if !text.is_empty() {
					// A normal message conventionally mentioning the bot.
					String::from(text)
				} else {
					// A message that had only a mention.
					return;
				};
				self.chatgpt
					.start_conversation(&self.database, context, text, message)
					.await;
			} else if let Some(referenced) = message.referenced_message.as_ref() {
				if referenced.author.id == own_id {
					// A message with no mention, replying to the bot.
					self.chatgpt
						.continue_conversation(&self.database, context, message)
						.await;
				}
			}
		}
	}

	async fn interaction_create(&self, context: Context, interaction: Interaction) {
		if let Interaction::ApplicationCommand(interaction) = interaction {
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
					let commands = guild
						.set_application_commands(&context.http, |commands| {
							commands
								.create_application_command(|command| allowances::register(command))
								.create_application_command(|command| {
									allowances::register_check_expenditure(command)
								})
						})
						.await
						.unwrap();

					let command_names = commands.into_iter().map(|command| command.name).join(", ");
					println!(
						"I now have the following guild slash commands in guild {}: {}",
						guild.as_u64(),
						command_names
					);
				}
			}
		}
	}
}
