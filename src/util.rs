use serenity::{
	all::CommandInteraction,
	builder::{CreateInteractionResponse, CreateInteractionResponseMessage},
	prelude::Context,
	Result,
};

pub async fn interaction_reply<S>(
	context: Context,
	interaction: CommandInteraction,
	content: S,
	ephemeral: bool,
) -> Result<()>
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
