use std::fmt::Display;

use serenity::{
	model::prelude::{application_command::ApplicationCommandInteraction, InteractionResponseType},
	prelude::Context,
	Result,
};

pub async fn interaction_reply<S>(
	context: Context,
	interaction: ApplicationCommandInteraction,
	content: S,
	ephemeral: bool,
) -> Result<()>
where
	S: Display,
{
	interaction
		.create_interaction_response(&context.http, |response| {
			response
				.kind(InteractionResponseType::ChannelMessageWithSource)
				.interaction_response_data(|message| message.content(content).ephemeral(ephemeral))
		})
		.await
}
