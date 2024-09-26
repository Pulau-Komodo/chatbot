use itertools::Itertools;
use serenity::{all::Cache, async_trait, model::prelude::*, prelude::*};
use sqlx::{query, Pool, Sqlite};

use crate::{allowances, chatgpt::Chatgpt, conversations::MessageIds, user_settings};

/// If there is a mention on either end of the string, removes it and trims. Removes only one mention.
fn strip_mention<'l>(text: &'l str, mentions: &[String]) -> Option<&'l str> {
	[str::strip_prefix, str::strip_suffix]
		.into_iter()
		.cartesian_product(mentions)
		.find_map(|(strip, mention)| strip(text, mention))
		.map(str::trim)
}

/// If there is a message link at the start of the string, removes it and trims the start, and returns both the remaining message and the IDs from the link.
fn extract_message_link(mut text: &str) -> Option<(&str, MessageIds)> {
	text = text.strip_prefix("https://")?;
	text = text
		.strip_prefix("ptb.")
		.or_else(|| text.strip_prefix("canary."))
		.unwrap_or(text);
	let text = text.strip_prefix("discord.com/channels/")?;
	let mut section = 0;
	let mut ids = [0, 0, 0];
	for (index, byte) in text.bytes().enumerate() {
		match byte {
			b'0'..=b'9' => {
				ids[section] *= 10;
				ids[section] += (byte - b'0') as u64;
			}
			b'/' if section < 2 => section += 1,
			b' ' if section == 2 && ids.into_iter().all(|id| id != 0) => {
				let text = text[index..].trim_start();
				let parent = MessageIds::new(
					GuildId::new(ids[0]),
					ChannelId::new(ids[1]),
					MessageId::new(ids[2]),
				);
				return Some((text, parent));
			}
			_ => {
				return None;
			}
		}
	}
	None
}

async fn get_message(
	context: &Context,
	channel_id: ChannelId,
	message_id: MessageId,
) -> Option<Message> {
	if let Some(message) = context.cache.message(channel_id, message_id) {
		return Some(message.to_owned());
	}
	context.http.get_message(channel_id, message_id).await.ok()
}

async fn get_referenced_contents(
	http: &std::sync::Arc<serenity::http::Http>,
	mut referenced: Message,
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

async fn is_own_message(executor: &Pool<Sqlite>, message_id: MessageId) -> bool {
	let message_id = message_id.get() as i64;
	query!(
		"
		SELECT count(*) AS is_own_message
		FROM conversations
		WHERE message = ?
		LIMIT 1
	",
		message_id
	)
	.fetch_one(executor)
	.await
	.unwrap()
	.is_own_message
		== 1
}

enum ReferencedMessage {
	None,
	Own(MessageIds),
	Others(MessageIds, String),
}

impl ReferencedMessage {
	async fn get_referenced_and_content<'l>(
		database: &Pool<Sqlite>,
		context: &Context,
		message: &mut Message,
		mut content: &'l str,
	) -> Option<(Self, &'l str)> {
		let message =
			if let Some(referenced) = std::mem::take(&mut message.referenced_message).map(|m| *m) {
				let referenced_ids = MessageIds::new(
					message.guild_id.unwrap(),
					referenced.channel_id,
					referenced.id,
				);
				if referenced.author.id == context.cache.current_user().id {
					ReferencedMessage::Own(referenced_ids)
				} else if let Some(referenced_contents) =
					get_referenced_contents(&context.http, referenced).await
				{
					ReferencedMessage::Others(referenced_ids, referenced_contents)
				} else {
					// It has a referenced message, but the bot couldn't get it.
					println!(
						"Could not get message referenced by message {}",
						message.id.get()
					);
					return None;
				}
			} else if let Some((text, link)) = extract_message_link(content) {
				if is_own_message(database, link.message_id).await {
					ReferencedMessage::Own(link)
				} else if let Some(mut linked_message) =
					get_message(context, link.channel_id, link.message_id).await
				{
					content = text;
					let referenced_ids = MessageIds::new(
						message.guild_id.unwrap(),
						linked_message.channel_id,
						linked_message.id,
					);
					if linked_message.author.id == context.cache.current_user().id {
						// Own message but should have found it in the database above.
						return None;
					} else {
						ReferencedMessage::Others(
							referenced_ids,
							std::mem::take(&mut linked_message.content),
						)
					}
				} else {
					ReferencedMessage::None
				}
			} else {
				ReferencedMessage::None
			};
		Some((message, content))
	}
	async fn get_parent_and_content(
		self,
		reply_body: &str,
		mentions: &[String],
	) -> Option<(Option<MessageIds>, String)> {
		let mut parent = None;
		let content = match self {
			Self::Own(referenced) => {
				parent = Some(referenced);
				reply_body.to_string()
			}
			Self::Others(_, referenced_contents) => {
				let Some(text) = strip_mention(reply_body, mentions) else {
					// Pinged the bot but had no mention at either end, so don't take it as being addressed.
					return None;
				};
				if text.is_empty() {
					// A message replying to something, but containing nothing but a mention to the bot
					// Stripping mentions so replies can be used to repeat queries, possibly with different settings.
					let referenced_contents = strip_mention(&referenced_contents, mentions)
						.map(str::to_string)
						.unwrap_or(referenced_contents);
					if referenced_contents.is_empty() {
						// Referenced message had only a mention, or otherwise no content (like only an image), makes no sense, ignore.
						return None;
					}
					referenced_contents
				} else {
					// A message replying to something, and containing its own text as well
					format!("{text} \"{referenced_contents}\"")
				}
			}
			Self::None => {
				let Some(text) = strip_mention(reply_body, mentions) else {
					// Pinged the bot but had no mention at either end, so don't take it as being addressed.
					return None;
				};
				if text.is_empty() {
					// Nothing other than a mention, ignore.
					return None;
				}
				text.to_string()
			}
		};
		Some((parent, content))
	}
	fn is_allowed_to_be_replied_to(
		&self,
		message: &Message,
		cache: &std::sync::Arc<Cache>,
	) -> bool {
		match self {
			Self::None => true,
			Self::Own(message_ids) => message_ids.is_allowed_to_be_replied_to(message, cache),
			Self::Others(message_ids, _) => message_ids.is_allowed_to_be_replied_to(message, cache),
		}
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

		let Some((referenced, content)) = ReferencedMessage::get_referenced_and_content(
			&self.database,
			&context,
			&mut message,
			&content,
		)
		.await
		else {
			return;
		};
		if !referenced.is_allowed_to_be_replied_to(&message, &context.cache) {
			return;
		}
		let Some((parent, content)) = referenced
			.get_parent_and_content(content, &self.mentions)
			.await
		else {
			return;
		};

		self.chatgpt
			.query(&self.database, context, content, message, parent)
			.await;
	}
}

#[async_trait]
impl EventHandler for DiscordEventHandler {
	async fn message(&self, context: Context, message: Message) {
		let own_id = context.cache.current_user().id;
		if message.author.id != own_id
			&& message.mentions_user_id(own_id)
			&& !message.content.is_empty()
		{
			self.handle_conversation_message(context, message).await;
		}
	}

	async fn interaction_create(&self, context: Context, interaction: Interaction) {
		if let Interaction::Command(interaction) = interaction {
			let _ = match interaction.data.name.as_str() {
				"allowance" => {
					allowances::command_check(
						context,
						interaction,
						&self.database,
						self.chatgpt.daily_allowance(),
						self.chatgpt.accrual_days(),
					)
					.await
				}
				"spent" => {
					allowances::command_expenditure(context, interaction, &self.database).await
				}
				"model" => {
					user_settings::command_set_model(
						context,
						interaction,
						&self.database,
						&self.chatgpt,
					)
					.await
				}
				"personality" => {
					user_settings::command_set_personality(context, interaction, &self.database)
						.await
				}
				name => {
					if let Some(one_off) = self.chatgpt.get_one_off_by_name(name) {
						one_off
							.handle(context, interaction, &self.chatgpt, &self.database)
							.await
					} else {
						eprintln!("Received unknown command: {}", name);
						Err(())
					}
				}
			};
		}
	}

	async fn ready(&self, context: Context, _ready: Ready) {
		println!("Ready");
		let arg = std::env::args().nth(1);
		if let Some(arg) = arg {
			if &arg == "register" {
				let mut command_count = 2 + self.chatgpt.one_offs().len();
				if !self.chatgpt.models().is_empty() {
					command_count += 1;
				}
				if self.chatgpt.personalities().len() > 1 {
					command_count += 1;
				}
				let mut commands = Vec::with_capacity(command_count);
				commands.extend([
					allowances::register(),
					allowances::register_check_expenditure(),
				]);
				if !self.chatgpt.models().is_empty() {
					commands.push(user_settings::register_set_model(&self.chatgpt));
				}
				if self.chatgpt.personalities().len() > 1 {
					commands.push(user_settings::register_set_personality(&self.chatgpt));
				}
				for one_off in self.chatgpt.one_offs() {
					commands.push(one_off.create());
				}
				for guild in context.cache.guilds() {
					let commands = guild
						.set_commands(&context.http, commands.clone())
						.await
						.unwrap();
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
