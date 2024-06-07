use serde::Deserialize;
use serenity::{
	all::{CommandInteraction, CommandOptionType, UserId},
	builder::{CreateCommand, CreateCommandOption},
	client::Context,
};
use sqlx::{Pool, Sqlite};

use crate::{
	allowances::{allowance_and_max, spend_allowance},
	chatgpt::{ChatMessage, Chatgpt},
	user_settings::consume_model_setting,
	util::{format_chatgpt_message, interaction_followup},
};

const TEMPERATURE: f32 = 0.5;
const MAX_TOKENS: u32 = 400;

#[derive(Debug, Clone, Deserialize)]
pub struct OneOffCommand {
	name: String,
	emoji: String,
	description: String,
	argument: String,
	argument_description: String,
	system_message: String,
}

impl OneOffCommand {
	pub fn name(&self) -> &str {
		&self.name
	}
	pub fn create(&self) -> CreateCommand {
		CreateCommand::new(&self.name)
			.description(&self.description)
			.add_option(
				CreateCommandOption::new(
					CommandOptionType::String,
					&self.argument,
					&self.argument_description,
				)
				.required(true),
			)
	}
	pub async fn handle(
		&self,
		context: Context,
		interaction: CommandInteraction,
		chatgpt: &Chatgpt,
		executor: &Pool<Sqlite>,
	) -> Result<(), ()> {
		single_text_input_with_system_message(
			context,
			interaction,
			chatgpt,
			executor,
			&self.emoji,
			&self.system_message,
		)
		.await
	}
}

impl Chatgpt {
	/// An OK result is a success response from the ChatGPT API. An error can be an error response from the API or an error before even sending to the API.
	async fn one_off(
		&self,
		executor: &Pool<Sqlite>,
		user: UserId,
		system_message: &str,
		emoji: &str,
		input: &str,
	) -> Result<String, String> {
		let custom_authorization_header = self.custom_authorization_header(user);

		let (allowance, max_allowance) = allowance_and_max(
			executor,
			user,
			self.daily_allowance(),
			self.accrual_days(),
			custom_authorization_header.is_some(),
		)
		.await;
		if allowance.is_out() {
			return Err(format!(
				"You are out of allowance. ({}/{})",
				allowance, max_allowance
			));
		}

		let model = consume_model_setting(executor, user)
			.await
			.and_then(|name| self.get_model_by_name(&name))
			.unwrap_or(self.default_model());

		let authorization_header =
			custom_authorization_header.unwrap_or(self.authorization_header());

		let response = self
			.send(
				&[
					ChatMessage::system(system_message.to_string()),
					ChatMessage::user(input.to_string()),
				],
				model.name(),
				TEMPERATURE,
				MAX_TOKENS,
				authorization_header,
			)
			.await?;

		let (allowance, cost) = spend_allowance(
			executor,
			user,
			response.usage.prompt_tokens,
			response.usage.completion_tokens,
			model,
			self.daily_allowance(),
			self.accrual_days(),
			custom_authorization_header.is_some(),
		)
		.await;

		Ok(format_chatgpt_message(
			&response.message_choices[0],
			emoji,
			cost,
			allowance,
			(self.default_model() != model).then_some(model),
		))
	}
}

async fn single_text_input_with_system_message(
	context: Context,
	interaction: CommandInteraction,
	chatgpt: &Chatgpt,
	executor: &Pool<Sqlite>,
	emoji: &str,
	system_message: &str,
) -> Result<(), ()> {
	let Some(input) = interaction
		.data
		.options
		.first()
		.and_then(|option| option.value.as_str())
	else {
		return Err(());
	};

	interaction.defer(&context).await.map_err(|_| ())?;

	let response = match chatgpt
		.one_off(executor, interaction.user.id, system_message, emoji, input)
		.await
	{
		Ok(response) => response,
		Err(error) => {
			let _ = interaction_followup(context, interaction, error, true).await;
			return Ok(());
		}
	};
	let _ = interaction_followup(context, interaction, response, false).await;
	Ok(())
}
