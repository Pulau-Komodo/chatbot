use serenity::{
	all::{CommandInteraction, CommandOptionType, UserId},
	builder::{CreateCommand, CreateCommandOption},
	client::Context,
};
use sqlx::{Pool, Sqlite};

use crate::{
	allowances::{
		check_allowance, get_max_allowance_millidollars, nanodollars_to_millidollars,
		spend_allowance,
	},
	chatgpt::{ChatMessage, Chatgpt},
	config::SystemMessages,
	util::{format_chatgpt_message, interaction_followup},
};

const TEMPERATURE: f32 = 0.5;
const MAX_TOKENS: u32 = 400;

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
		let allowance =
			check_allowance(executor, user, self.daily_allowance(), self.accrual_days()).await;
		let max_allowance =
			get_max_allowance_millidollars(self.daily_allowance(), self.accrual_days()).await;
		if allowance <= 0 {
			return Err(format!(
				"You are out of allowance. ({}m$/{}m$)",
				nanodollars_to_millidollars(allowance),
				max_allowance
			));
		}

		let response = self
			.send(
				&[
					ChatMessage::system(system_message.to_string()),
					ChatMessage::user(input.to_string()),
				],
				self.default_model().name(),
				TEMPERATURE,
				MAX_TOKENS,
			)
			.await?;

		let (allowance, cost) = spend_allowance(
			executor,
			user,
			response.usage.prompt_tokens,
			response.usage.completion_tokens,
			self.default_model(),
			self.daily_allowance(),
			self.accrual_days(),
		)
		.await;

		Ok(format_chatgpt_message(
			&response.message_choices[0],
			emoji,
			cost,
			allowance,
			None,
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

pub async fn command_dictionary(
	context: Context,
	interaction: CommandInteraction,
	chatgpt: &Chatgpt,
	system_messages: &SystemMessages,
	executor: &Pool<Sqlite>,
) -> Result<(), ()> {
	single_text_input_with_system_message(
		context,
		interaction,
		chatgpt,
		executor,
		"📖",
		&system_messages.dictionary,
	)
	.await
}

pub fn create_command_dictionary() -> CreateCommand {
	CreateCommand::new("gptdictionary")
		.description("Provides a dictionary entry for the given term.")
		.add_option(
			CreateCommandOption::new(
				CommandOptionType::String,
				"term",
				"The term to get a dictionary entry for.",
			)
			.required(true),
		)
}

pub async fn command_judgment(
	context: Context,
	interaction: CommandInteraction,
	chatgpt: &Chatgpt,
	system_messages: &SystemMessages,
	executor: &Pool<Sqlite>,
) -> Result<(), ()> {
	single_text_input_with_system_message(
		context,
		interaction,
		chatgpt,
		executor,
		"👨‍⚖️",
		&system_messages.judgment,
	)
	.await
}

pub fn create_command_judgment() -> CreateCommand {
	CreateCommand::new("judgment")
		.description("Judges the specified crime.")
		.add_option(
			CreateCommandOption::new(
				CommandOptionType::String,
				"crime",
				"The crime to have judged.",
			)
			.required(true),
		)
}
