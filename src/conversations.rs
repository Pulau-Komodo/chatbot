use serenity::{
	model::prelude::{Message, MessageId},
	prelude::Context,
};
use sqlx::{query, Pool, Sqlite};
use std::fmt::Write;

use crate::{
	allowances::{check_allowance, nanodollars_to_millidollars, spend_allowance, MAX_MILLIDOLLARS},
	chatgpt::{ChatGptModel, ChatMessage, Chatgpt, Role},
	response_styles::SystemMessage,
	user_settings::{consume_model_setting, get_system_message},
};

const MODEL: ChatGptModel = ChatGptModel::Gpt35Turbo;
const TEMPERATURE: f32 = 0.5;
const MAX_TOKENS: u32 = 200;

impl Chatgpt {
	/// Start or continue a conversation, based on the presence of `parent_id`.
	pub async fn query(
		&self,
		executor: &Pool<Sqlite>,
		context: Context,
		input: String,
		message: Message,
		parent_id: Option<MessageId>,
	) {
		let allowance = check_allowance(executor, message.author.id).await;
		if allowance <= 0 {
			let reply = format!(
				"You are out of allowance. ({}m$/{}m$)",
				nanodollars_to_millidollars(allowance),
				MAX_MILLIDOLLARS
			);
			message.reply(context.http, reply).await.unwrap();
			return;
		}

		let (history, system_message, system_message_was_set) = if let Some(parent_id) = parent_id {
			let system_message = get_message_system_message(executor, parent_id).await;
			let system_message_was_set = system_message.is_some();
			let system_message = system_message.unwrap_or_default();
			let mut history =
				get_history_from_database(executor, parent_id, system_message.text()).await;
			if history.len() == 1 {
				// Found no actual history, so ignore this message. This most typically happens when replying to a bot message that was not a GPT response, like an error message.
				return;
			}
			history.push(ChatMessage {
				role: Role::User,
				content: input.clone(),
			});
			(history, system_message, system_message_was_set)
		} else {
			let system_message = get_system_message(executor, message.author.id).await;
			let system_message_was_set = system_message.is_some();
			let system_message = system_message.unwrap_or_default();
			let history = [
				ChatMessage {
					role: Role::System,
					content: system_message.text(),
				},
				ChatMessage {
					role: Role::User,
					content: input.clone(),
				},
			]
			.to_vec();
			(history, system_message, system_message_was_set)
		};

		let model = consume_model_setting(executor, message.author.id)
			.await
			.unwrap_or(MODEL);

		let mut response = match self.send(&history, model, TEMPERATURE, MAX_TOKENS).await {
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
		)
		.await;

		let output = std::mem::take(&mut response.message_choices[0].message.content);
		let mut full_reply = format!(
			"{} {} (-{} m$, {} m$)",
			system_message.emoji(),
			output,
			nanodollars_to_millidollars(cost),
			nanodollars_to_millidollars(allowance),
		);
		if !matches!(model, ChatGptModel::Gpt35Turbo) {
			write!(full_reply, " ({})", model.as_friendly_str()).unwrap(); // Add non-standard model to the message
		}
		let own_message = message.reply(context.http, full_reply).await.unwrap();

		let system_message = system_message_was_set.then_some(system_message);
		if let Some(parent_id) = parent_id {
			store_child_message(
				executor,
				own_message.id,
				parent_id,
				&input,
				&output,
				system_message,
			)
			.await;
		} else {
			store_root_message(executor, own_message.id, &input, &output, system_message).await;
		}
	}
}

async fn get_history_from_database(
	executor: &Pool<Sqlite>,
	parent: MessageId,
	system_message: String,
) -> Vec<ChatMessage> {
	let message_id = *parent.as_u64() as i64;
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
	std::iter::once(ChatMessage {
		role: Role::System,
		content: system_message,
	})
	.chain(stored_history.into_iter().rev().flat_map(|record| {
		[
			ChatMessage {
				role: Role::User,
				content: record.input,
			},
			ChatMessage {
				role: Role::Assistant,
				content: record.output,
			},
		]
	}))
	.collect()
}

async fn get_message_system_message(
	executor: &Pool<Sqlite>,
	parent: MessageId,
) -> Option<SystemMessage> {
	let message_id = parent.0 as i64;
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
	.map(|message| SystemMessage::from_database_str(&message))
}

async fn store_root_message(
	executor: &Pool<Sqlite>,
	message: MessageId,
	input: &str,
	output: &str,
	system_message: Option<SystemMessage>,
) {
	let message_id = message.0 as i64;
	let system_message = system_message.map(|message| message.to_database_string());
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
	system_message: Option<SystemMessage>,
) {
	let message_id = message.0 as i64;
	let parent_id = parent.0 as i64;
	let system_message = system_message.map(|message| message.to_database_string());
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
