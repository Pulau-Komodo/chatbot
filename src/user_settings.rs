use serenity::{
	all::{CommandInteraction, CommandOptionType, UserId},
	builder::{CreateCommand, CreateCommandOption},
	prelude::Context,
};
use sqlx::{query, Pool, Sqlite};

use crate::{chatgpt::ChatgptModel, response_styles::SystemMessage, util::interaction_reply};

// Model

async fn get_model(executor: &Pool<Sqlite>, user: UserId) -> Option<ChatgptModel> {
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
	.and_then(|record| record.model.map(|model| model.try_into().unwrap()))
}

async fn set_model(executor: &Pool<Sqlite>, user: UserId, model: Option<ChatgptModel>) {
	let user_id = user.get() as i64;
	let model = model.map(|model| model.as_str());
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

pub async fn consume_model_setting(executor: &Pool<Sqlite>, user: UserId) -> Option<ChatgptModel> {
	let model_setting = get_model(executor, user).await;
	if model_setting.is_some() {
		set_model(executor, user, None).await;
	}
	model_setting
}

pub async fn command_set_gpt4(
	context: Context,
	interaction: CommandInteraction,
	executor: &Pool<Sqlite>,
) -> Result<(), ()> {
	let current_model = get_model(executor, interaction.user.id).await;
	let new_model = current_model.xor(Some(ChatgptModel::Gpt4));
	set_model(executor, interaction.user.id, new_model).await;
	let output = match new_model {
		Some(model) => format!("Model for the next message set to {}.", model),
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

// System message

pub async fn get_system_message(executor: &Pool<Sqlite>, user: UserId) -> Option<SystemMessage> {
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
			.map(|message| SystemMessage::from_database_str(&message))
	})
}

async fn set_system_message(
	executor: &Pool<Sqlite>,
	user: UserId,
	system_message: Option<SystemMessage>,
) {
	let user_id = user.get() as i64;
	let system_message = system_message.map(|message| message.to_database_string());
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

pub async fn command_set_system_message(
	context: Context,
	interaction: CommandInteraction,
	executor: &Pool<Sqlite>,
) -> Result<(), ()> {
	let current_system_message = get_system_message(executor, interaction.user.id).await;
	let new_system_message = interaction
		.data
		.options
		.first()
		.and_then(|option| option.value.as_str())
		.map(SystemMessage::from_database_str);

	if current_system_message == new_system_message {
		let _ = interaction_reply(
			context,
			interaction,
			"Your system message is already set to that.",
			true,
		)
		.await;
		return Ok(());
	}
	let name = new_system_message.as_ref().map(|message| message.name());
	set_system_message(executor, interaction.user.id, new_system_message).await;
	let output = match name {
		Some(name) => format!("System message for future new conversations set to {name}."),
		None => String::from("System message for future new conversations reset to default."),
	};
	let _ = interaction_reply(context, interaction, output, true).await;
	Ok(())
}

pub fn register_set_system_message() -> CreateCommand {
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
