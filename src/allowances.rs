use std::ops::Add;

use chrono::{DateTime, Duration, Utc};
use serenity::all::{CommandInteraction, CommandOptionType};
use serenity::builder::{CreateCommand, CreateCommandOption};
use serenity::{model::prelude::UserId, prelude::Context};
use sqlx::{query, Pool, Sqlite};

use crate::chatgpt::ChatgptModel;
use crate::util::interaction_reply;

/// The allowance a user gets over time each day, in nanodollars.
const DAILY_ALLOWANCE: u32 = 20_000_000;
/// The allowance a user can save up before it stops accruing, in nanodollars.
const MAX_ALLOWANCE: u32 = DAILY_ALLOWANCE * 2;

const MILLISECONDS_PER_DAY: f32 = 1000.0 * 60.0 * 60.0 * 24.0;

async fn time_to_full(executor: &Pool<Sqlite>, user: UserId) -> Option<DateTime<Utc>> {
	let user_id = user.get() as i64;
	let result = query!(
		"
		SELECT
			time_to_full
		FROM
			allowances
		WHERE
			user = ?
		",
		user_id
	)
	.fetch_optional(executor)
	.await
	.unwrap();
	result
		.map(|record| DateTime::from_naive_utc_and_offset(record.time_to_full, Utc).max(Utc::now()))
}

fn allowance_from_time_to_full(time_to_full: DateTime<Utc>) -> i32 {
	let duration = time_to_full - Utc::now();
	let days_left = duration.num_milliseconds() as f32 / MILLISECONDS_PER_DAY;
	let missing_allowance = days_left * DAILY_ALLOWANCE as f32;
	(MAX_ALLOWANCE as f32 - missing_allowance) as i32
}

pub async fn check_allowance(executor: &Pool<Sqlite>, user: UserId) -> i32 {
	let time = time_to_full(executor, user).await;
	if let Some(time) = time {
		allowance_from_time_to_full(time)
	} else {
		MAX_ALLOWANCE as i32
	}
}

/// Get the cost of a query in nanodollars.
pub fn get_cost(input_tokens: u32, output_tokens: u32, model: ChatgptModel) -> u32 {
	match model {
		ChatgptModel::Gpt35Turbo => 1_500 * input_tokens + 2_000 * output_tokens,
		ChatgptModel::Gpt4 => 30_000 * input_tokens + 60_000 * output_tokens,
		_ => unimplemented!("Other models are not suppported"),
	}
}

/// Takes the specified number of tokens' worth from the user's allowance, then returns the new allowance and what the cost ended up being.
pub async fn spend_allowance(
	executor: &Pool<Sqlite>,
	user: UserId,
	input_tokens: u32,
	output_tokens: u32,
	model: ChatgptModel,
) -> (i32, i32) {
	let cost = get_cost(input_tokens, output_tokens, model);

	let added_milliseconds = cost as u64 * 1000 * 60 * 60 * 24 / DAILY_ALLOWANCE as u64;
	let time = time_to_full(executor, user).await.unwrap_or_else(Utc::now);
	let new_time = time.add(Duration::milliseconds(added_milliseconds as i64));
	let user_id = user.get() as i64;

	query!(
		"
		INSERT INTO
			allowances (user, time_to_full)
		VALUES
			(?, ?)
		",
		user_id,
		new_time,
	)
	.execute(executor)
	.await
	.unwrap();

	let model = model.as_str();
	query!(
		"
		INSERT INTO
			spending (user, cost, input_tokens, output_tokens, model)
		VALUES
			(?, ?, ?, ?, ?)
		",
		user_id,
		cost,
		input_tokens,
		output_tokens,
		model,
	)
	.execute(executor)
	.await
	.unwrap();

	(allowance_from_time_to_full(new_time), cost as i32)
}

const PRECISION_MULTIPLIER: f32 = 100.0;
const MILLIDOLLARS_PER_NANODOLLAR: f32 = 1.0e6;
/// The allowance in millidollars, for strings.
pub const MAX_MILLIDOLLARS: f32 = MAX_ALLOWANCE as f32 / MILLIDOLLARS_PER_NANODOLLAR;

/// Converts an integer number of nanodollars to a float number of millidollars, rounded to 2 decimal places.
pub fn nanodollars_to_millidollars(allowance: i32) -> f32 {
	let millidollars = allowance as f32 / MILLIDOLLARS_PER_NANODOLLAR;
	(millidollars * PRECISION_MULTIPLIER).round() / PRECISION_MULTIPLIER
}

pub async fn command_check(
	context: Context,
	interaction: CommandInteraction,
	executor: &Pool<Sqlite>,
) -> Result<(), ()> {
	let allowance = check_allowance(executor, interaction.user.id).await;
	let millidollars = nanodollars_to_millidollars(allowance);
	let content = format!(
		"You have {} out of {} millidollars left.",
		millidollars, MAX_MILLIDOLLARS
	);
	interaction_reply(context, interaction, content, false)
		.await
		.unwrap();
	Ok(())
}
pub fn register() -> CreateCommand {
	CreateCommand::new("allowance").description("Check your current allowance for using ChatGPT.")
}

async fn get_expenditure(executor: &Pool<Sqlite>, user: Option<UserId>) -> u32 {
	if let Some(user) = user {
		let user_id = user.get() as i64;
		query!(
			"
		SELECT
			SUM(cost) as cost
		FROM
			spending
		WHERE user = ?
		",
			user_id
		)
		.fetch_one(executor)
		.await
		.unwrap()
		.cost
		.map(|n| n as u32)
	} else {
		query!(
			"
			SELECT
				SUM(cost) as cost
			FROM
				spending
			",
		)
		.fetch_one(executor)
		.await
		.unwrap()
		.cost
		.map(|n| n as u32)
	}
	.unwrap_or(0)
}

pub async fn command_expenditure(
	context: Context,
	interaction: CommandInteraction,
	executor: &Pool<Sqlite>,
) -> Result<(), ()> {
	let all = interaction
		.data
		.options
		.get(0)
		.and_then(|option| option.value.as_bool())
		.unwrap_or(false);
	let expenditure = get_expenditure(executor, (!all).then_some(interaction.user.id)).await;
	let millidollars = nanodollars_to_millidollars(expenditure as i32);
	let content = if !all {
		format!("You have used {} millidollars.", millidollars)
	} else {
		format!("Everyone combined has used {} millidollars.", millidollars)
	};
	interaction_reply(context, interaction, content, false)
		.await
		.unwrap();
	Ok(())
}
pub fn register_check_expenditure() -> CreateCommand {
	CreateCommand::new("spent")
		.description(
			"Check how many millidollars you have or everyone has used on ChatGPT prompts.",
		)
		.add_option(
			CreateCommandOption::new(
				CommandOptionType::Boolean,
				"all",
				"Get total spending from everyone",
			)
			.required(false),
		)
}
