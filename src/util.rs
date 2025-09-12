use serenity::{
	all::{CommandInteraction, Message},
	builder::{
		CreateEmbed, CreateInteractionResponse, CreateInteractionResponseFollowup,
		CreateInteractionResponseMessage, CreateMessage,
	},
	constants,
	http::Http,
	prelude::{Context, SerenityError},
};

use crate::{
	allowances::Allowance,
	gpt::{GptModel, MessageChoice},
};

/// Replies to a message, without pinging, putting the text into an embed if it's too long.
pub async fn reply<S>(message: Message, http: &Http, content: S) -> Result<Message, SerenityError>
where
	S: Into<String>,
{
	let content: String = content.into();
	let message_builder = CreateMessage::new().reference_message(&message);
	if content.chars().count() <= constants::MESSAGE_CODE_LIMIT {
		message
			.channel_id
			.send_message(http, message_builder.content(content))
			.await
	} else {
		message
			.channel_id
			.send_message(
				http,
				message_builder.add_embed(CreateEmbed::new().description(content)),
			)
			.await
	}
}

/// Replies to an interaction, putting the text into an embed if it's too long.
pub async fn interaction_reply<S>(
	context: Context,
	interaction: CommandInteraction,
	content: S,
	ephemeral: bool,
) -> Result<(), SerenityError>
where
	S: Into<String>,
{
	let content: String = content.into();
	if content.chars().count() <= constants::MESSAGE_CODE_LIMIT {
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
	} else {
		interaction
			.create_response(
				&context.http,
				CreateInteractionResponse::Message(
					CreateInteractionResponseMessage::new()
						.embed(CreateEmbed::new().description(content))
						.ephemeral(ephemeral),
				),
			)
			.await
	}
}

/// Follows up on an interaction reply (typically a defer), putting the text into an embed if it's too long.
pub async fn interaction_followup<S>(
	context: Context,
	interaction: CommandInteraction,
	content: S,
	ephemeral: bool,
	always_embed: bool,
) -> Result<(), SerenityError>
where
	S: Into<String>,
{
	let content: String = content.into();
	if !always_embed && content.chars().count() <= constants::MESSAGE_CODE_LIMIT {
		interaction
			.create_followup(
				&context.http,
				CreateInteractionResponseFollowup::new()
					.content(content)
					.ephemeral(ephemeral),
			)
			.await
	} else {
		interaction
			.create_followup(
				&context.http,
				CreateInteractionResponseFollowup::new()
					.embed(CreateEmbed::new().description(content))
					.ephemeral(ephemeral),
			)
			.await
	}
	.map(|_| ())
}

/// Attaches formatting to the message from GPT, like "🤖 Hello. (-0.25 m$, 39.95 m$) (GPT-4)".
pub fn format_chat_message(
	response: &MessageChoice,
	emoji: &str,
	cost: Allowance,
	allowance: Allowance,
	model: Option<&GptModel>,
) -> String {
	let output = &response.message.content;
	let ending = ending_from_finish_reason(&response.finish_reason);
	if let Some(model) = model {
		format!(
			"{} {}{} (-{}, {}) ({})",
			emoji,
			output,
			ending,
			cost,
			allowance,
			model.friendly_name(),
		)
	} else {
		format!("{} {}{} (-{}, {})", emoji, output, ending, cost, allowance,)
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
