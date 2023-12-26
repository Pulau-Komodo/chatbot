use serenity::{
	all::{CommandInteraction, CommandOptionType, UserId},
	builder::{CreateCommand, CreateCommandOption},
	client::Context,
};
use sqlx::{Pool, Sqlite};

use crate::{
	allowances::{check_allowance, nanodollars_to_millidollars, spend_allowance, MAX_MILLIDOLLARS},
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
		input: &str,
	) -> Result<String, String> {
		let allowance = check_allowance(executor, user).await;
		if allowance <= 0 {
			return Err(format!(
				"You are out of allowance. ({}m$/{}m$)",
				nanodollars_to_millidollars(allowance),
				MAX_MILLIDOLLARS
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
		)
		.await;

		Ok(format_chatgpt_message(
			&response.message_choices[0],
			"📖",
			cost,
			allowance,
			None,
		))
	}
}

pub async fn command_dictionary(
	context: Context,
	interaction: CommandInteraction,
	chatgpt: &Chatgpt,
	executor: &Pool<Sqlite>,
) -> Result<(), ()> {
	const DICTIONARY_MESSAGE: &str = "You are a terse dictionary. The user will provide a word or phrase, and you need to explain what it means. If you do not know the word or phrase, invent a plausible-sounding fictitious meaning. Your reply needs to be formatted like an abridged dictionary entry. If the user input is not a word or a phrase but, for example, a whole sentence or question, just reply that their input is invalid.";

	let Some(term) = interaction
		.data
		.options
		.first()
		.and_then(|option| option.value.as_str())
	else {
		return Err(());
	};

	interaction.defer(&context).await.map_err(|_| ())?;

	let response = match chatgpt
		.one_off(executor, interaction.user.id, DICTIONARY_MESSAGE, term)
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
