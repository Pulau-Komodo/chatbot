use serenity::{
	model::prelude::{Message, MessageId},
	prelude::Context,
};
use sqlx::{query, Pool, Sqlite};

use crate::{
	allowances::{check_allowance, nanodollars_to_millidollars, spend_allowance, MAX_MILLIDOLLARS},
	chatgpt::{ChatGptModel, ChatMessage, Chatgpt, Role},
};

const SYSTEM_MESSAGE: &str = "You are a computer assistant. Reply tersely and robotically.";
const MODEL: ChatGptModel = ChatGptModel::Gpt35Turbo;
const TEMPERATURE: f32 = 0.5;
const MAX_TOKENS: u32 = 200;

impl Chatgpt {
	pub async fn start_conversation(
		&self,
		executor: &Pool<Sqlite>,
		context: Context,
		input: String,
		message: Message,
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

		let history = [
			ChatMessage {
				role: Role::System,
				content: String::from(SYSTEM_MESSAGE),
			},
			ChatMessage {
				role: Role::User,
				content: input.clone(),
			},
		];

		let mut response = match self.send(&history, MODEL, TEMPERATURE, MAX_TOKENS).await {
			Ok(response) => response,
			Err(error_message) => {
				message.reply(context.http, error_message).await.unwrap();
				return;
			}
		};

		let allowance = spend_allowance(
			executor,
			message.author.id,
			response.usage.prompt_tokens,
			response.usage.completion_tokens,
		)
		.await;
		let output = std::mem::take(&mut response.message_choices[0].message.content);
		let full_reply = format!("{} ({} m$)", output, nanodollars_to_millidollars(allowance),);
		let own_message = message.reply(context.http, full_reply).await.unwrap();
		let message_id = *own_message.id.as_u64() as i64;

		query!(
			"
			INSERT INTO
				conversations (message, input, output)
			VALUES
				(?, ?, ?)
			",
			message_id,
			input,
			output
		)
		.execute(executor)
		.await
		.unwrap();
	}

	pub async fn continue_conversation(
		&self,
		executor: &Pool<Sqlite>,
		context: Context,
		message: Message,
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

		let parent_id = message.referenced_message.as_ref().unwrap().id;
		let mut history = get_history_from_database(executor, parent_id).await;
		if history.len() == 1 {
			// Found no actual history, so ignore this message.
			return;
		}
		history.push(ChatMessage {
			role: Role::User,
			content: message.content.clone(),
		});

		let mut response = match self.send(&history, MODEL, TEMPERATURE, MAX_TOKENS).await {
			Ok(response) => response,
			Err(error_message) => {
				message.reply(context.http, error_message).await.unwrap();
				return;
			}
		};

		let allowance = spend_allowance(
			executor,
			message.author.id,
			response.usage.prompt_tokens,
			response.usage.completion_tokens,
		)
		.await;
		let output = std::mem::take(&mut response.message_choices[0].message.content);
		let full_reply = format!("{} ({} m$)", output, nanodollars_to_millidollars(allowance),);
		let own_message = message.reply(context.http, full_reply).await.unwrap();
		let message_id = *own_message.id.as_u64() as i64;
		let parent_id = *parent_id.as_u64() as i64;

		query!(
			"
			INSERT INTO
				conversations (message, parent, input, output)
			VALUES
				(?, ?, ?, ?)
			",
			message_id,
			parent_id,
			message.content,
			output
		)
		.execute(executor)
		.await
		.unwrap();
	}
}

async fn get_history_from_database(executor: &Pool<Sqlite>, parent: MessageId) -> Vec<ChatMessage> {
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
		content: String::from(SYSTEM_MESSAGE),
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
