use serenity::{
	model::prelude::{Message, MessageId},
	prelude::Context,
};
use sqlx::{query, Pool, Sqlite};

use crate::{
	allowances::{
		check_allowance, get_max_allowance_millidollars, nanodollars_to_millidollars,
		spend_allowance,
	},
	chatgpt::{ChatMessage, Chatgpt, ChatgptModel},
	config::SystemMessages,
	response_styles::Personality,
	user_settings::{consume_model_setting, get_user_personality},
	util::{format_chatgpt_message, reply},
};

const DEFAULT_MODEL: ChatgptModel = ChatgptModel::Gpt35Turbo;
const TEMPERATURE: f32 = 0.5;
const MAX_TOKENS: u32 = 400;

impl Chatgpt {
	/// Start or continue a conversation, based on the presence of `parent_id`.
	pub async fn query(
		&self,
		executor: &Pool<Sqlite>,
		system_messages: &SystemMessages,
		context: Context,
		input: String,
		message: Message,
		parent_id: Option<MessageId>,
	) {
		let allowance = check_allowance(
			executor,
			message.author.id,
			self.daily_allowance(),
			self.accrual_days(),
		)
		.await;
		let max_millidollars =
			get_max_allowance_millidollars(self.daily_allowance(), self.accrual_days()).await;
		if allowance <= 0 {
			let reply = format!(
				"You are out of allowance. ({}m$/{}m$)",
				nanodollars_to_millidollars(allowance),
				max_millidollars
			);
			message.reply(context.http, reply).await.unwrap();
			return;
		}

		let (history, personality) = if let Some(parent_id) = parent_id {
			let Some(values) =
				continue_conversation(executor, parent_id, system_messages, &input).await
			else {
				// Parent not found.
				return;
			};
			values
		} else {
			start_conversation(executor, &message, system_messages, &input).await
		};

		let model = consume_model_setting(executor, message.author.id)
			.await
			.unwrap_or(DEFAULT_MODEL);

		let response = match self.send(&history, model, TEMPERATURE, MAX_TOKENS).await {
			Ok(response) => response,
			Err(error_message) => {
				message.reply(context.http, error_message).await.unwrap();
				return;
			}
		};

		let (allowance, cost) = spend_allowance(
			executor,
			message.author.id,
			response.usage.prompt_tokens,
			response.usage.completion_tokens,
			model,
			self.daily_allowance(),
			self.accrual_days(),
		)
		.await;

		let full_reply = format_chatgpt_message(
			&response.message_choices[0],
			personality.emoji(),
			cost,
			allowance,
			(model != DEFAULT_MODEL).then_some(model),
		);
		let output = &response.message_choices[0].message.content;
		let own_message = reply(message, &context.http, full_reply).await.unwrap();

		if let Some(parent_id) = parent_id {
			store_child_message(
				executor,
				own_message.id,
				parent_id,
				&input,
				output,
				personality,
			)
			.await;
		} else {
			store_root_message(executor, own_message.id, &input, output, personality).await;
		}
	}
}

/// Attempt to continue an existing conversation from a reply.
async fn continue_conversation(
	executor: &Pool<Sqlite>,
	parent_id: MessageId,
	system_messages: &SystemMessages,
	input: &str,
) -> Option<(Vec<ChatMessage>, Personality)> {
	let personality = get_message_personality(executor, parent_id)
		.await
		.unwrap_or_default();
	let system_message = system_messages.personality_message(&personality);
	let mut history =
		get_history_from_database(executor, parent_id, system_message.to_string()).await;
	if history.len() == 1 {
		// Found no actual history, so ignore this message. This most typically happens when replying to a bot message that was not a GPT response, like an error message.
		return None;
	}
	history.push(ChatMessage::user(input.to_string()));
	Some((history, personality))
}

/// Start a new conversation.
async fn start_conversation(
	executor: &Pool<Sqlite>,
	message: &Message,
	system_messages: &SystemMessages,
	input: &str,
) -> (Vec<ChatMessage>, Personality) {
	let personality = get_user_personality(executor, message.author.id)
		.await
		.unwrap_or_default();
	let system_message = system_messages.personality_message(&personality);
	let history = [
		ChatMessage::system(system_message.to_string()),
		ChatMessage::user(input.to_string()),
	]
	.to_vec();
	(history, personality)
}

async fn get_history_from_database(
	executor: &Pool<Sqlite>,
	parent: MessageId,
	system_message: String,
) -> Vec<ChatMessage> {
	let message_id = parent.get() as i64;
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
				WHERE message = ?
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
		message_id
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

async fn get_message_personality(
	executor: &Pool<Sqlite>,
	parent: MessageId,
) -> Option<Personality> {
	let message_id = parent.get() as i64;
	query!(
		"
		SELECT
			system_message
		FROM
			conversations
		WHERE
			message = ?
		",
		message_id
	)
	.fetch_optional(executor)
	.await
	.unwrap()
	.and_then(|record| record.system_message)
	.map(|message| Personality::from_database_str(&message))
}

async fn store_root_message(
	executor: &Pool<Sqlite>,
	message: MessageId,
	input: &str,
	output: &str,
	personality: Personality,
) {
	let message_id = message.get() as i64;
	let system_message = personality.to_database_string();
	query!(
		"
		INSERT INTO
			conversations (message, input, output, system_message)
		VALUES
			(?, ?, ?, ?)
		",
		message_id,
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
	message: MessageId,
	parent: MessageId,
	input: &str,
	output: &str,
	personality: Personality,
) {
	let message_id = message.get() as i64;
	let parent_id = parent.get() as i64;
	let system_message = personality.to_database_string();
	query!(
		"
		INSERT INTO
			conversations (message, parent, input, output, system_message)
		VALUES
			(?, ?, ?, ?, ?)
		",
		message_id,
		parent_id,
		input,
		output,
		system_message,
	)
	.execute(executor)
	.await
	.unwrap();
}
