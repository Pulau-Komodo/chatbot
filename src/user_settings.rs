use serenity::{
	all::{CommandInteraction, CommandOptionType, UserId},
	builder::{CreateCommand, CreateCommandOption},
	prelude::Context,
};
use sqlx::{query, Pool, Sqlite};

use crate::{chatgpt::Chatgpt, util::interaction_reply};

// Model

async fn get_model(executor: &Pool<Sqlite>, user: UserId) -> Option<String> {
	let user_id = user.get() as i64;
	query!(
		"
		SELECT
			model
		FROM
			user_settings
		WHERE
			user = ?
		",
		user_id
	)
	.fetch_optional(executor)
	.await
	.unwrap()
	.and_then(|record| record.model)
}

async fn set_model(executor: &Pool<Sqlite>, user: UserId, model: Option<&str>) {
	let user_id = user.get() as i64;
	query!(
		"
		INSERT INTO
			user_settings (user, model)
		VALUES
			(?, ?)
		ON CONFLICT (user)
			DO UPDATE SET
				model = excluded.model
		",
		user_id,
		model
	)
	.execute(executor)
	.await
	.unwrap();
}

pub async fn consume_model_setting(executor: &Pool<Sqlite>, user: UserId) -> Option<String> {
	let model_setting = get_model(executor, user).await;
	if model_setting.is_some() {
		set_model(executor, user, None).await;
	}
	model_setting
}

/// Set the model to be used for a single response.
pub async fn command_set_model(
	context: Context,
	interaction: CommandInteraction,
	executor: &Pool<Sqlite>,
	chatgpt: &Chatgpt,
) -> Result<(), ()> {
	let current_model_name = get_model(executor, interaction.user.id)
		.await
		.unwrap_or(chatgpt.default_model().name().to_string());
	let new_model_name = interaction
		.data
		.options
		.first()
		.unwrap()
		.value
		.as_str()
		.unwrap();
	let new_model = chatgpt.get_model_by_name(new_model_name).unwrap(); // To do: handle this more gracefully. It will panic if the database still has some model that later became unsupported.
	let output = if current_model_name == new_model_name {
		format!(
			"Model was already set to {} ({}).",
			new_model.friendly_name(),
			new_model.get_cost_description()
		)
	} else {
		if new_model == chatgpt.default_model() {
			set_model(executor, interaction.user.id, None).await;
		} else {
			set_model(executor, interaction.user.id, Some(new_model.name())).await;
		}
		format!(
			"Model for your next prompt set to {} ({}).",
			new_model.friendly_name(),
			new_model.get_cost_description()
		)
	};
	let _ = interaction_reply(context, interaction, output, true).await;
	Ok(())
}

pub fn register_set_model(chatgpt: &Chatgpt) -> CreateCommand {
	let mut model_option = CreateCommandOption::new(
		CommandOptionType::String,
		"model",
		"The model to use for your next prompt.",
	)
	.required(true)
	.add_string_choice(
		format!("{} (default)", chatgpt.default_model().friendly_name()),
		chatgpt.default_model().name(),
	);
	for model in chatgpt.models() {
		model_option = model_option.add_string_choice(model.friendly_name(), model.name());
	}

	CreateCommand::new("model")
		.description("Sets the model to use for your next prompt.")
		.add_option(model_option)
}

// Personality

/// Get the chat personality set for the specified user.
pub async fn get_user_personality(executor: &Pool<Sqlite>, user: UserId) -> Option<String> {
	let user_id = user.get() as i64;
	query!(
		"
		SELECT
			system_message
		FROM
			user_settings
		WHERE
			user = ?
		",
		user_id
	)
	.fetch_optional(executor)
	.await
	.unwrap()
	.and_then(|record| record.system_message)
}

async fn set_personality(executor: &Pool<Sqlite>, user: UserId, personality: Option<&str>) {
	let user_id = user.get() as i64;
	query!(
		"
		INSERT INTO
			user_settings (user, system_message)
		VALUES
			(?, ?)
		ON CONFLICT (user)
			DO UPDATE SET
				system_message = excluded.system_message
		",
		user_id,
		personality,
	)
	.execute(executor)
	.await
	.unwrap();
}

pub async fn command_set_personality(
	context: Context,
	interaction: CommandInteraction,
	executor: &Pool<Sqlite>,
) -> Result<(), ()> {
	let current_personality = get_user_personality(executor, interaction.user.id).await;
	let new_personality = interaction
		.data
		.options
		.first()
		.and_then(|option| option.value.as_str());

	if current_personality.as_deref() == new_personality {
		let _ = interaction_reply(
			context,
			interaction,
			"The personality is already set to that.",
			true,
		)
		.await;
		return Ok(());
	}
	set_personality(executor, interaction.user.id, new_personality).await;
	let output = match new_personality {
		Some(name) => format!("Personality for future new conversations set to {name}."),
		None => String::from("Personality for future new conversations reset to default."),
	};
	let _ = interaction_reply(context, interaction, output, true).await;
	Ok(())
}

pub fn register_set_personality(chatgpt: &Chatgpt) -> CreateCommand {
	let mut personality_option = CreateCommandOption::new(
		CommandOptionType::String,
		"personality",
		"The personality your new conversations will use.",
	)
	.required(true);
	for personality in chatgpt.personalities() {
		personality_option =
			personality_option.add_string_choice(personality.name(), personality.name());
	}

	CreateCommand::new("personality")
		.description("Sets (or unsets) the personality for new conversations started by you.")
		.add_option(personality_option)
}
