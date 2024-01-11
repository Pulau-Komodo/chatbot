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
	chatgpt::{ChatMessage, Chatgpt, ChatgptModel},
	util::{format_chatgpt_message, interaction_followup},
};

const MODEL: ChatgptModel = ChatgptModel::Gpt35Turbo;
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
				MODEL,
				TEMPERATURE,
				MAX_TOKENS,
			)
			.await?;

		let (allowance, cost) = spend_allowance(
			executor,
			user,
			response.usage.prompt_tokens,
			response.usage.completion_tokens,
			MODEL,
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
	executor: &Pool<Sqlite>,
) -> Result<(), ()> {
	const DICTIONARY_MESSAGE: &str = "You are a terse dictionary. The user will provide a word or phrase, and you need to explain what it means. If you do not know the word or phrase, invent a plausible-sounding fictitious meaning. Your reply needs to be formatted like an abridged dictionary entry.";

	single_text_input_with_system_message(
		context,
		interaction,
		chatgpt,
		executor,
		"📖",
		DICTIONARY_MESSAGE,
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
	executor: &Pool<Sqlite>,
) -> Result<(), ()> {
	const JUDGMENT_MESSAGE: &str = "You are a royal judge with medieval views on punishment. The user will tell you a moral or social transgression, and you need to come up with a creative and unusual punishment that relates to the crime. For example, annoying drunkards may be told to drink a lot, or they may be made to walk the streets wearing only a barrel. If what the user said is totally fine morally and socially, instead of coming up with a punishment, just tell them it's not a crime.";

	single_text_input_with_system_message(
		context,
		interaction,
		chatgpt,
		executor,
		"👨‍⚖️",
		JUDGMENT_MESSAGE,
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
