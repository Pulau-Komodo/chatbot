use serenity::{
	builder::CreateApplicationCommand,
	model::prelude::{application_command::ApplicationCommandInteraction, UserId},
	prelude::Context,
};
use sqlx::{query, Pool, Sqlite};

use crate::{chatgpt::ChatGptModel, util::interaction_reply};

async fn get_model(executor: &Pool<Sqlite>, user: UserId) -> Option<ChatGptModel> {
	let user_id = user.0 as i64;
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

async fn set_model(executor: &Pool<Sqlite>, user: UserId, model: Option<ChatGptModel>) {
	let user_id = user.0 as i64;
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

pub async fn consume_model_setting(executor: &Pool<Sqlite>, user: UserId) -> Option<ChatGptModel> {
	let model_setting = get_model(executor, user).await;
	if model_setting.is_some() {
		set_model(executor, user, None).await;
	}
	model_setting
}

pub async fn command_set_gpt4(
	context: Context,
	interaction: ApplicationCommandInteraction,
	executor: &Pool<Sqlite>,
) -> Result<(), ()> {
	let current_model = get_model(executor, interaction.user.id).await;
	let new_model = current_model.xor(Some(ChatGptModel::Gpt4));
	set_model(executor, interaction.user.id, new_model).await;
	let output = match new_model {
		Some(model) => format!("Model for the next message set to {}.", model),
		None => String::from("Model reset to default."),
	};
	let _ = interaction_reply(context, interaction, output, true).await;
	Ok(())
}
pub fn register_set_gpt4(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
	command
		.name("gpt4")
		.description(
			"Sets (or unsets) your next prompt to use GPT-4, a fancier model with 20 to 30 times the cost.",
		)
}
