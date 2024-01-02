use serenity::{
	all::CommandInteraction,
	builder::{
		CreateInteractionResponse, CreateInteractionResponseFollowup,
		CreateInteractionResponseMessage,
	},
	prelude::Context,
};

use crate::{
	allowances::nanodollars_to_millidollars,
	chatgpt::{ChatgptModel, MessageChoice},
};

pub async fn interaction_reply<S>(
	context: Context,
	interaction: CommandInteraction,
	content: S,
	ephemeral: bool,
) -> serenity::Result<()>
where
	S: Into<String>,
{
	interaction
		.create_response(
			&context.http,
			CreateInteractionResponse::Message(
				CreateInteractionResponseMessage::new()
					.content(content)
					.ephemeral(ephemeral),
			),
		)
		.await
}

pub async fn interaction_followup<S>(
	context: Context,
	interaction: CommandInteraction,
	content: S,
	ephemeral: bool,
) -> Result<(), ()>
where
	S: Into<String>,
{
	interaction
		.create_followup(
			&context.http,
			CreateInteractionResponseFollowup::new()
				.content(content)
				.ephemeral(ephemeral),
		)
		.await
		.map_err(|_| ())
		.map(|_| ())
}

/// Attaches formatting to the message from ChatGPT, like "🤖 Hello. (-0.25 m$, 39.95 m$) (GPT-4)".
pub fn format_chatgpt_message(
	response: &MessageChoice,
	emoji: &str,
	cost: i32,
	allowance: i32,
	model: Option<ChatgptModel>,
) -> String {
	let output = &response.message.content;
	let ending = ending_from_finish_reason(&response.finish_reason);
	let cost = nanodollars_to_millidollars(cost);
	let allowance = nanodollars_to_millidollars(allowance);
	if let Some(model) = model {
		format!(
			"{} {}{} (-{} m$, {} m$) ({})",
			emoji,
			output,
			ending,
			cost,
			allowance,
			model.friendly_str(),
		)
	} else {
		format!(
			"{} {}{} (-{} m$, {} m$)",
			emoji, output, ending, cost, allowance,
		)
	}
}

pub fn ending_from_finish_reason(finish_reason: &str) -> &'static str {
	match finish_reason {
		// It was done.
		"stop" => "",
		// It got cut off by the token limit.
		"length" => "…",
		// Omitted content due to content filters.
		"content_filter" => " \\🙊",
		// "function call" should only happen if the AI decides to call a function, "null" means "API response still in progress or incomplete", and other options are not listed.
		reason => {
			eprintln!("GPT API somehow returned finish reason \"{reason}\".");
			"⁇"
		}
	}
}
