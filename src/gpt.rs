//! I used this as a starting point: https://github.com/Maxuss/chatgpt_rs Copyright (c) 2022 Maksim Petrov
//! But there is almost nothing left of it.

use reqwest::{
	header::{HeaderValue, AUTHORIZATION},
	Url,
};
use serde::{Deserialize, Serialize};
use serenity::all::{RoleId, UserId};
use std::{collections::HashMap, fmt::Display};

use crate::{
	config::{Config, CustomApiKeys},
	one_off_response::OneOffCommand,
	response_styles::{extract_custom, Personality, PersonalityPreset},
};

// The client that operates the GPT API
#[derive(Debug, Clone)]
pub struct Gpt {
	client: reqwest::Client,
	api_url: Url,
	authorization_header: HeaderValue,
	custom_authorization_headers: HashMap<UserId, HeaderValue>,
	config: Config,
}

impl Gpt {
	/// Constructs a new GPT API client with provided API key and URL.
	///
	/// `api_url` is the URL of the /v1/chat/completions endpoint. Can be used to set a proxy.
	pub fn new<S>(
		api_key: S,
		api_url: Option<Url>,
		config: Config,
		custom_api_keys: CustomApiKeys,
	) -> Result<Self, ()>
	where
		S: Display,
	{
		let api_url = api_url
			.unwrap_or_else(|| Url::parse("https://api.openai.com/v1/chat/completions").unwrap());

		let authorization_header =
			HeaderValue::from_bytes(format!("Bearer {api_key}").as_bytes()).unwrap();
		let client = reqwest::Client::new();

		Ok(Self {
			client,
			api_url,
			authorization_header,
			custom_authorization_headers: custom_api_keys.into_headers(),
			config,
		})
	}

	/// Sends a conversation to the API and gets the next message.
	pub async fn send(
		&self,
		history: &[ChatMessage],
		model: &str,
		temperature: f32,
		max_tokens: u32,
		authorization_header: &HeaderValue,
	) -> Result<CompletionResponse, String> {
		let response = self
			.client
			.post(self.api_url.clone())
			.header(AUTHORIZATION, authorization_header)
			.json(&CompletionRequest {
				model,
				messages: history,
				stream: false,
				temperature,
				top_p: 1.0,
				frequency_penalty: 0.0,
				presence_penalty: 0.0,
				reply_count: 1,
				max_tokens,
			})
			.send()
			.await
			.map_err(|error| {
				println!("{error}");
				String::from("Boop beep, problem sending request.")
			})?;
		// let response_text = response.text().await;
		// println!("{:?}", response_text);
		// return Err("Testing".into());
		let response_debug = format!("{:?}", response);
		let response: ServerResponse = response.json().await.map_err(|error| {
			println!("{error}, {:?}", response_debug);
			String::from("Boop beep, problem deserialising response.")
		})?;
		match response {
			ServerResponse::Error { error } => {
				eprintln!("Backend error: {}, {}", error.message, error.error_type);
				let text = match error.error_type.as_str() {
					"insufficient_quota" => "Boop bloop, out of credit.",
					"server_error" => "Boop bloop, server error.",
					"requests" => "Beep bloop, probably rate-limited.",
					_ => "Boop bloop, unknown error",
				};
				Err(String::from(text))
			}
			ServerResponse::Completion(completion) => {
				if [
					completion
						.usage
						.completion_tokens_details
						.accepted_prediction_tokens,
					completion
						.usage
						.completion_tokens_details
						.rejected_prediction_tokens,
					completion.usage.completion_tokens_details.audio_tokens,
					completion.usage.completion_tokens_details.audio_tokens,
					completion.usage.prompt_tokens_details.audio_tokens,
					completion.usage.prompt_tokens_details.cached_tokens,
				]
				.iter()
				.any(|tokens| *tokens != 0)
				{
					println!("{}", completion.message_choices[0].message.content);
					println!("{:?}", completion.usage);
				}
				Ok(completion)
			}
		}
	}
	pub fn authorization_header(&self) -> &HeaderValue {
		&self.authorization_header
	}
	pub fn custom_authorization_header(&self, user: UserId) -> Option<&HeaderValue> {
		self.custom_authorization_headers.get(&user)
	}
	pub fn daily_allowance(&self) -> u32 {
		self.config.daily_allowance
	}
	pub fn accrual_days(&self) -> f32 {
		self.config.accrual_days
	}
	pub fn get_model_by_name(&self, name: &str) -> Option<&GptModel> {
		self.config.models.iter().find(|model| model.name() == name)
	}
	pub fn default_model(&self) -> &GptModel {
		self.config.models.first().unwrap() // There should always be at least one model, enforced on creating `Config`.
	}
	/// The available models, excluding default.
	pub fn models(&self) -> &Vec<GptModel> {
		&self.config.models
	}
	pub fn get_personality_by_name<'a>(&'a self, name: &str) -> Option<Personality<'a>> {
		if let Some(message) = extract_custom(name) {
			Some(Personality::Custom(message.to_string()))
		} else {
			self.config
				.personalities
				.iter()
				.find(|personality| personality.name() == name)
				.map(Personality::Preset)
		}
	}
	pub fn default_personality(&self) -> &PersonalityPreset {
		self.config.personalities.first().unwrap() // There should always be at least one personality, enforced on creating `Config`.
	}
	pub fn personalities(&self) -> &Vec<PersonalityPreset> {
		&self.config.personalities
	}
	pub fn get_one_off_by_name(&self, name: &str) -> Option<&OneOffCommand> {
		self.config
			.one_offs
			.iter()
			.find(|one_off| one_off.name() == name)
	}
	pub fn one_offs(&self) -> &Vec<OneOffCommand> {
		&self.config.one_offs
	}
	pub fn prototyping_roles(&self) -> &Vec<RoleId> {
		&self.config.prototyping_roles
	}
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct GptModel {
	name: String,
	friendly_name: String,
	input_cost: u32,
	output_cost: u32,
}

impl GptModel {
	/// Name as used by the API and the database.
	pub fn name(&self) -> &str {
		&self.name
	}
	/// Name to display to users.
	pub fn friendly_name(&self) -> &str {
		&self.friendly_name
	}
	/// Get the cost of a query in nanodollars.
	pub fn get_cost(&self, tokens: TokenUsage) -> u32 {
		self.input_cost * tokens.prompt_tokens + self.output_cost * tokens.completion_tokens
	}
	/// Get a description of the cost of this model.
	pub fn get_cost_description(&self) -> String {
		format!(
			"{}$ per 1M input tokens, {}$ per 1M output tokens",
			self.input_cost as f32 / 1000.0,
			self.output_cost as f32 / 1000.0
		)
	}
}

/// A role of a message sender, can be:
/// - `System`, for starting system message, that sets the tone of model
/// - `Assistant`, for messages sent by GPT
/// - `User`, for messages sent by user
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize, Eq, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Role {
	/// A system message, automatically sent at the start to set the tone of the model
	System,
	/// A message sent by GPT
	Assistant,
	/// A message sent by the user
	User,
}

/// Container for the sent/received GPT messages
#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct ChatMessage {
	/// Role of message sender
	pub role: Role,
	/// Actual content of the message
	pub content: String,
}

impl ChatMessage {
	pub fn system(content: String) -> Self {
		Self {
			role: Role::System,
			content,
		}
	}
	pub fn assistant(content: String) -> Self {
		Self {
			role: Role::Assistant,
			content,
		}
	}
	pub fn user(content: String) -> Self {
		Self {
			role: Role::User,
			content,
		}
	}
}

/// A request struct sent to the API to request a message completion
#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize)]
pub struct CompletionRequest<'a> {
	/// The model to be used, currently `gpt-3.5-turbo`, but may change in future
	pub model: &'a str,
	/// The message history, including the message that requires completion, which should be the last one
	pub messages: &'a [ChatMessage],
	/// Whether the message response should be gradually streamed
	pub stream: bool,
	/// The extra randomness of response
	pub temperature: f32,
	/// Controls diversity via nucleus sampling, not recommended to use with temperature
	pub top_p: f32,
	/// Determines how much to penalize new tokens based on their existing frequency so far
	pub frequency_penalty: f32,
	/// Determines how much to penalize new tokens pased on their existing presence so far
	pub presence_penalty: f32,
	/// Determines the number of output responses
	#[serde(rename = "n")]
	pub reply_count: u32,
	/// The maximum number of tokens to generate in the chat completion
	pub max_tokens: u32,
}

/// Represents a response from the API
#[derive(Debug, Clone, PartialEq, PartialOrd, Deserialize)]
#[serde(untagged)]
pub enum ServerResponse {
	/// An error occurred, most likely the model was just overloaded
	Error {
		/// The error that happened
		error: CompletionError,
	},
	/// Completion successfuly completed
	Completion(CompletionResponse),
}

/// An error happened while requesting completion
#[derive(Debug, Clone, PartialEq, PartialOrd, Deserialize)]
pub struct CompletionError {
	/// Message, describing the error
	pub message: String,
	/// The type of error. Example: `server_error`
	#[serde(rename = "type")]
	pub error_type: String,
}

/// A response struct received from the API after requesting a message completion
#[derive(Debug, Clone, PartialEq, PartialOrd, Deserialize)]
pub struct CompletionResponse {
	/// Unique ID of the message, but not in a UUID format.
	/// Example: `chatcmpl-6p5FEv1JHictSSnDZsGU4KvbuBsbu`
	#[serde(rename = "id")]
	pub message_id: Option<String>,
	/// Unix seconds timestamp of when the response was created
	#[serde(rename = "created")]
	pub created_timestamp: Option<u64>,
	/// The model that was used for this completion
	pub model: String,
	/// Token usage of this completion
	pub usage: TokenUsage,
	/// Message choices for this response, guaranteed to contain at least one message response
	#[serde(rename = "choices")]
	pub message_choices: Vec<MessageChoice>,
}

/// A message completion choice struct
#[derive(Debug, Clone, PartialEq, PartialOrd, Deserialize)]
pub struct MessageChoice {
	/// The actual message
	pub message: ChatMessage,
	/// The reason completion was stopped
	pub finish_reason: String,
	/// The index of this message in the outer `message_choices` array
	pub index: u32,
}

/// The token usage of a specific response
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Deserialize)]
pub struct TokenUsage {
	/// Tokens spent on the prompt message (including previous messages)
	pub prompt_tokens: u32,
	/// Tokens spent on the completion message
	pub completion_tokens: u32,
	/// Total amount of tokens used (`prompt_tokens + completion_tokens`)
	pub total_tokens: u32,
	/// "Breakdown of tokens used in a completion."
	pub completion_tokens_details: CompletionTokenDetails,
	/// "Breakdown of tokens used in the prompt."
	pub prompt_tokens_details: PromptTokenDetails,
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Deserialize)]
pub struct PromptTokenDetails {
	/// "Cached tokens present in the prompt."
	pub cached_tokens: u32,
	/// "Audio input tokens present in the prompt."
	pub audio_tokens: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Deserialize)]
pub struct CompletionTokenDetails {
	/// "Tokens generated by the model for reasoning."
	pub reasoning_tokens: u32,
	/// "Audio input tokens generated by the model."
	pub audio_tokens: u32,
	/// "When using Predicted Outputs, the number of tokens in the prediction that appeared in the completion."
	pub accepted_prediction_tokens: u32,
	/// "When using Predicted Outputs, the number of tokens in the prediction that did not appear in the completion. However, like reasoning tokens, these tokens are still counted in the total completion tokens for purposes of billing, output, and context window limits."
	pub rejected_prediction_tokens: u32,
}
