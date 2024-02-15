use serenity::{
	all::{CommandInteraction, CommandOptionType, UserId},
	builder::{CreateCommand, CreateCommandOption},
	prelude::Context,
};
use sqlx::{query, Pool, Sqlite};

use crate::{chatgpt::Chatgpt, response_styles::Personality, util::interaction_reply};

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

/// Note that this feature is hardcoded to "gpt-4", in spite of the configurability of the model in config. Any actual change of the model name in config will currently break the feature.
/// To do: fix this one way or the other.
pub async fn command_set_gpt4(
	context: Context,
	interaction: CommandInteraction,
	executor: &Pool<Sqlite>,
	chatgpt: &Chatgpt,
) -> Result<(), ()> {
	let current_model = get_model(executor, interaction.user.id).await;
	let new_model = current_model.xor(Some(String::from("gpt-4")));
	set_model(executor, interaction.user.id, new_model.as_deref()).await;
	let output = match new_model {
		Some(model) => {
			let friendly_name = chatgpt
				.get_model_by_name(&model)
				.map_or(model.as_str(), |model| model.friendly_name());
			format!("Model for the next message set to {}.", friendly_name)
		}
		None => String::from("Model reset to default."),
	};
	let _ = interaction_reply(context, interaction, output, true).await;
	Ok(())
}

pub fn register_set_gpt4() -> CreateCommand {
	CreateCommand::new("gpt4").description(
		"Sets (or unsets) your next prompt to use GPT-4, a fancier model with 20 to 30 times the cost.",
	)
}

// Personality

/// Get the chat personality set for the specified user.
pub async fn get_user_personality(executor: &Pool<Sqlite>, user: UserId) -> Option<Personality> {
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
	.and_then(|record| {
		record
			.system_message
			.map(|message| Personality::from_database_str(&message))
	})
}

async fn set_personality(executor: &Pool<Sqlite>, user: UserId, personality: Option<Personality>) {
	let user_id = user.get() as i64;
	let system_message = personality.map(|message| message.to_database_string());
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
		system_message,
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
		.and_then(|option| option.value.as_str())
		.map(Personality::from_database_str);

	if current_personality == new_personality {
		let _ = interaction_reply(
			context,
			interaction,
			"The personality is already set to that.",
			true,
		)
		.await;
		return Ok(());
	}
	let name = new_personality
		.as_ref()
		.map(|personality| personality.name());
	set_personality(executor, interaction.user.id, new_personality).await;
	let output = match name {
		Some(name) => format!("Personality for future new conversations set to {name}."),
		None => String::from("Personality for future new conversations reset to default."),
	};
	let _ = interaction_reply(context, interaction, output, true).await;
	Ok(())
}

pub fn register_set_personality() -> CreateCommand {
	CreateCommand::new("personality")
		.description("Sets (or unsets) the personality for new conversations started by you.")
		.add_option(
			CreateCommandOption::new(
				CommandOptionType::String,
				"personality",
				"The personality your new conversations will use.",
			)
			.add_string_choice("robotic", "robotic")
			.add_string_choice("friendly", "friendly")
			.add_string_choice("poetic", "poetic")
			.add_string_choice("villainous", "villainous")
			.required(true),
		)
}
