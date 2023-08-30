//! Much of this was taken from https://github.com/Maxuss/chatgpt_rs Copyright (c) 2022 Maksim Petrov
//! But I have completely gutted and refactored it, removing all the parts I don't use, and shaping it to a different interaction model, where no conversation is stored outside the database, and options are not stored with the client.

use reqwest::{
	header::{HeaderMap, HeaderValue, AUTHORIZATION},
	Url,
};
use serde::{Deserialize, Serialize};
use std::fmt::Display;

// The client that operates the ChatGPT API
#[derive(Debug, Clone)]
pub struct Chatgpt {
	client: reqwest::Client,
	api_url: Url,
}

impl Chatgpt {
	/// Constructs a new ChatGPT API client with provided API key and URL.
	///
	/// `api_url` is the URL of the /v1/chat/completions endpoint. Can be used to set a proxy.
	pub fn new<S>(api_key: S, api_url: Option<Url>) -> Result<Self, ()>
	where
		S: Display,
	{
		let api_url = api_url
			.unwrap_or_else(|| Url::parse("https://api.openai.com/v1/chat/completions").unwrap());
		let mut headers = HeaderMap::new();
		headers.insert(
			AUTHORIZATION,
			HeaderValue::from_bytes(format!("Bearer {api_key}").as_bytes()).unwrap(),
		);
		let client = reqwest::ClientBuilder::new()
			.default_headers(headers)
			.build()
			.unwrap();
		Ok(Self { client, api_url })
	}

	/// Sends a conversation to the API and gets the next message.
	pub async fn send(
		&self,
		history: &[ChatMessage],
		model: ChatGptModel,
		temperature: f32,
		max_tokens: u32,
	) -> Result<CompletionResponse, String> {
		let response: ServerResponse = self
			.client
			.post(self.api_url.clone())
			.json(&CompletionRequest {
				model: model.as_str(),
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
			})?
			.json()
			.await
			.map_err(|error| {
				println!("{error}");
				String::from("Boop beep, problem derialising response.")
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
			ServerResponse::Completion(completion) => Ok(completion),
		}
	}
}

/// The engine version for ChatGPT
#[derive(Debug, Default, Copy, Clone, PartialEq, PartialOrd)]
#[allow(non_camel_case_types)]
pub enum ChatGptModel {
	/// Standard engine: `gpt-3.5-turbo`
	#[default]
	Gpt35Turbo,
	/// Different version of standard engine: `gpt-3.5-turbo-0301`
	_Gpt35Turbo_0301,
	/// Base GPT-4 model: `gpt-4`
	Gpt4,
	/// Version of GPT-4, able to remember 32,000 tokens: `gpt-4-32k`
	_Gpt4_32k,
	/// Different version of GPT-4: `gpt-4-0314`
	_Gpt4_0314,
	/// Different version of GPT-4, able to remember 32,000 tokens: `gpt-4-32k-0314`
	_Gpt4_32k_0314,
	/// Custom (or new/unimplemented) version of ChatGPT
	_Custom(&'static str),
}

impl TryInto<ChatGptModel> for String {
	type Error = ();

	fn try_into(self) -> Result<ChatGptModel, Self::Error> {
		let model = match self.as_str() {
			"gpt-3.5-turbo" => ChatGptModel::Gpt35Turbo,
			"gpt-3.5-turbo-030" => ChatGptModel::_Gpt35Turbo_0301,
			"gpt-4" => ChatGptModel::Gpt4,
			"gpt-4-32k" => ChatGptModel::_Gpt4_32k,
			"gpt-4-0314" => ChatGptModel::_Gpt4_0314,
			"gpt-4-32k-0314" => ChatGptModel::_Gpt4_32k_0314,
			_ => return Err(()),
		};
		Ok(model)
	}
}

impl Display for ChatGptModel {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(self.as_str())
	}
}

impl ChatGptModel {
	pub fn as_str(&self) -> &'static str {
		match self {
			Self::Gpt35Turbo => "gpt-3.5-turbo",
			Self::_Gpt35Turbo_0301 => "gpt-3.5-turbo-0301",
			Self::Gpt4 => "gpt-4",
			Self::_Gpt4_32k => "gpt-4-32k",
			Self::_Gpt4_0314 => "gpt-4-0314",
			Self::_Gpt4_32k_0314 => "gpt-4-32k-0314",
			Self::_Custom(custom) => custom,
		}
	}
	pub fn as_friendly_str(&self) -> &'static str {
		match self {
			Self::Gpt35Turbo => "GPT-3.5 Turbo",
			Self::_Gpt35Turbo_0301 => "GPT-3.5 Turbo 0301",
			Self::Gpt4 => "GPT-4",
			Self::_Gpt4_32k => "GPT-4 32k context",
			Self::_Gpt4_0314 => "GPT-4 0314",
			Self::_Gpt4_32k_0314 => "GPT-4 32k context 0314",
			Self::_Custom(custom) => custom,
		}
	}
}

/// A role of a message sender, can be:
/// - `System`, for starting system message, that sets the tone of model
/// - `Assistant`, for messages sent by ChatGPT
/// - `User`, for messages sent by user
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize, Eq, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Role {
	/// A system message, automatically sent at the start to set the tone of the model
	System,
	/// A message sent by ChatGPT
	Assistant,
	/// A message sent by the user
	User,
}

/// Container for the sent/received ChatGPT messages
#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct ChatMessage {
	/// Role of message sender
	pub role: Role,
	/// Actual content of the message
	pub content: String,
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
#[derive(Debug, Clone, PartialEq, PartialOrd, Deserialize)]
pub struct TokenUsage {
	/// Tokens spent on the prompt message (including previous messages)
	pub prompt_tokens: u32,
	/// Tokens spent on the completion message
	pub completion_tokens: u32,
	/// Total amount of tokens used (`prompt_tokens + completion_tokens`)
	pub total_tokens: u32,
}
