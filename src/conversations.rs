use std::sync::Arc;

use serenity::{
	all::{Cache, ChannelId, GuildId},
	model::prelude::{Message, MessageId},
	prelude::Context,
};
use sqlx::{query, Pool, Sqlite};

use crate::{
	allowances::{allowance_and_max, spend_allowance},
	chatgpt::{ChatMessage, Chatgpt},
	response_styles::Personality,
	user_settings::{get_model_setting, get_user_personality},
	util::{format_chatgpt_message, reply},
};

const TEMPERATURE: f32 = 0.5;
const MAX_TOKENS: u32 = 400;

impl Chatgpt {
	/// Start or continue a conversation, based on the presence of `parent`.
	pub async fn query(
		&self,
		executor: &Pool<Sqlite>,
		context: Context,
		input: String,
		message: Message,
		parent: Option<ParentMessage>,
	) {
		if let Some(parent) = parent {
			if !parent.is_allowed(&message, &context.cache) {
				return;
			}
		}

		let custom_authorization_header = self.custom_authorization_header(message.author.id);

		let (allowance, max_allowance) = allowance_and_max(
			executor,
			message.author.id,
			self.daily_allowance(),
			self.accrual_days(),
			custom_authorization_header.is_some(),
		)
		.await;
		if allowance.is_out() {
			let reply = format!(
				"You are out of allowance. ({}/{})",
				allowance, max_allowance
			);
			message.reply(context.http, reply).await.unwrap();
			return;
		}

		let (history, personality) = if let Some(parent_id) = parent {
			let Some(values) = self
				.continue_conversation(executor, parent_id, &input)
				.await
			else {
				// Parent not found.
				return;
			};
			values
		} else {
			self.start_conversation(executor, &message, &input).await
		};

		let model = get_model_setting(executor, message.author.id)
			.await
			.and_then(|name| {
				let model = self.get_model_by_name(&name);
				if model.is_none() {
					println!("Warning: could not get model by name of {name}.");
				}
				model
			})
			.unwrap_or(self.default_model());

		let authorization_header =
			custom_authorization_header.unwrap_or(self.authorization_header());

		let response = match self
			.send(
				&history,
				model.name(),
				TEMPERATURE,
				MAX_TOKENS,
				authorization_header,
			)
			.await
		{
			Ok(response) => response,
			Err(error_message) => {
				message.reply(context.http, error_message).await.unwrap();
				return;
			}
		};

		let (allowance, cost) = spend_allowance(
			executor,
			message.author.id,
			response.usage,
			model,
			self.daily_allowance(),
			self.accrual_days(),
			custom_authorization_header.is_some(),
		)
		.await;

		let guild_id = message.guild_id.unwrap();

		let full_reply = format_chatgpt_message(
			&response.message_choices[0],
			personality.emoji(),
			cost,
			allowance,
			(model.name() != self.default_model().name()).then_some(model),
		);
		let output = &response.message_choices[0].message.content;
		let own_message = reply(message, &context.http, full_reply).await.unwrap();

		if let Some(parent) = parent {
			store_child_message(
				executor,
				&own_message,
				guild_id,
				parent,
				&input,
				output,
				personality,
			)
			.await;
		} else {
			store_root_message(
				executor,
				&own_message,
				guild_id,
				&input,
				output,
				personality,
			)
			.await;
		}
	}

	/// Start a new conversation.
	async fn start_conversation(
		&self,
		executor: &Pool<Sqlite>,
		message: &Message,
		input: &str,
	) -> (Vec<ChatMessage>, &Personality) {
		let personality = get_user_personality(executor, message.author.id)
			.await
			.and_then(|per| self.get_personality_by_name(&per))
			.unwrap_or(self.default_personality());
		let history = [
			ChatMessage::system(personality.system_message().to_string()),
			ChatMessage::user(input.to_string()),
		]
		.to_vec();
		(history, personality)
	}

	/// Attempt to continue an existing conversation from a reply.
	async fn continue_conversation(
		&self,
		executor: &Pool<Sqlite>,
		parent: ParentMessage,
		input: &str,
	) -> Option<(Vec<ChatMessage>, &Personality)> {
		let personality = get_message_personality(executor, parent)
			.await
			.and_then(|per| self.get_personality_by_name(&per))
			.unwrap_or(self.default_personality());
		let mut history =
			get_history_from_database(executor, parent, personality.system_message().to_string())
				.await;
		if history.len() == 1 {
			// Found no actual history, so ignore this message. This most typically happens when replying to a bot message that was not a GPT response, like an error message.
			return None;
		}
		history.push(ChatMessage::user(input.to_string()));
		Some((history, personality))
	}
}

async fn get_history_from_database(
	executor: &Pool<Sqlite>,
	parent: ParentMessage,
	system_message: String,
) -> Vec<ChatMessage> {
	let (guild_id, channel_id, message_id) = parent.as_i64s();
	let stored_history = query!(
		"
		WITH RECURSIVE chain (
			next,
			input_n,
			output_n
		)
		AS (
			SELECT parent,
				input,
				output
			FROM conversations
			WHERE message = ? AND channel = ? AND guild = ?
			UNION ALL
			SELECT parent,
				input,
				output
			FROM chain,
				conversations
			WHERE message = next
			LIMIT 20
		)
		SELECT input_n AS input,
			output_n AS output
		FROM chain;
		",
		message_id,
		channel_id,
		guild_id
	)
	.fetch_all(executor)
	.await
	.unwrap();
	std::iter::once(ChatMessage::system(system_message))
		.chain(stored_history.into_iter().rev().flat_map(|record| {
			[
				ChatMessage::user(record.input),
				ChatMessage::assistant(record.output),
			]
		}))
		.collect()
}

async fn get_message_personality(executor: &Pool<Sqlite>, parent: ParentMessage) -> Option<String> {
	let (guild_id, channel_id, message_id) = parent.as_i64s();
	query!(
		"
		SELECT
			system_message
		FROM
			conversations
		WHERE
			message = ? AND channel = ? AND guild = ?
		",
		message_id,
		channel_id,
		guild_id,
	)
	.fetch_optional(executor)
	.await
	.unwrap()
	.and_then(|record| record.system_message)
}

async fn store_root_message(
	executor: &Pool<Sqlite>,
	message: &Message,
	guild_id: GuildId,
	input: &str,
	output: &str,
	personality: &Personality,
) {
	let message_id = message.id.get() as i64;
	let channel_id = message.channel_id.get() as i64;
	let guild_id = guild_id.get() as i64;
	let system_message = personality.name();
	query!(
		"
		INSERT INTO
			conversations (message, channel, guild, input, output, system_message)
		VALUES
			(?, ?, ?, ?, ?, ?)
		",
		message_id,
		channel_id,
		guild_id,
		input,
		output,
		system_message,
	)
	.execute(executor)
	.await
	.unwrap();
}

async fn store_child_message(
	executor: &Pool<Sqlite>,
	message: &Message,
	guild_id: GuildId,
	parent: ParentMessage,
	input: &str,
	output: &str,
	personality: &Personality,
) {
	let message_id = message.id.get() as i64;
	let channel_id = message.channel_id.get() as i64;
	let guild_id = guild_id.get() as i64;
	let parent_id = parent.message_id.get() as i64;
	let system_message = personality.name();
	query!(
		"
		INSERT INTO
			conversations (message, channel, guild, parent, input, output, system_message)
		VALUES
			(?, ?, ?, ?, ?, ?, ?)
		",
		message_id,
		channel_id,
		guild_id,
		parent_id,
		input,
		output,
		system_message,
	)
	.execute(executor)
	.await
	.unwrap();
}

#[derive(Debug, Clone, Copy)]
pub struct ParentMessage {
	pub guild_id: GuildId,
	pub channel_id: ChannelId,
	pub message_id: MessageId,
}

impl ParentMessage {
	pub fn new(guild_id: GuildId, channel_id: ChannelId, message_id: MessageId) -> Self {
		Self {
			guild_id,
			channel_id,
			message_id,
		}
	}
	/// Whether this parent is allowed for this user in this context. A parent is allowed when the guild is the same and the user linking it has access to view the channel.
	fn is_allowed(&self, message: &Message, cache: &Arc<Cache>) -> bool {
		if self.guild_id != message.guild_id.unwrap() {
			// Cross-guild replying is not allowed.
			false
		} else if self.channel_id != message.channel_id {
			let Some(guild) = message.guild(cache) else {
				return false;
			};
			let Some(channel) = guild.channels.get(&self.channel_id) else {
				return false;
			};
			guild
				.partial_member_permissions_in(
					channel,
					message.author.id,
					message.member.as_ref().unwrap(),
				)
				.view_channel()
		} else {
			true
		}
	}
	/// This is how they are stored in the database, out of necessity. Returns guild ID, channel ID and message ID, in that order.
	pub fn as_i64s(self) -> (i64, i64, i64) {
		(
			self.guild_id.get() as i64,
			self.channel_id.get() as i64,
			self.message_id.get() as i64,
		)
	}
}
