use std::borrow::Cow;

use serde::Deserialize;

#[derive(Debug, Clone)]
pub enum Personality<'p> {
	Preset(&'p PersonalityPreset),
	Custom(String),
}

impl Personality<'_> {
	/// Get a descriptor for the personality for use in messages that say what is set.
	pub fn name(&self) -> &str {
		match self {
			Self::Preset(p) => p.name(),
			Self::Custom(_) => "custom",
		}
	}
	/// The string identifying this personality in the database.
	pub fn database_name(&'_ self) -> Cow<'_, str> {
		match self {
			Self::Preset(_) => Cow::Borrowed(self.name()),
			Self::Custom(message) => Cow::Owned(wrap_custom(message)),
		}
	}
	/// Get the emoji that the bot will use to convey the used personality.
	pub fn emoji(&self) -> &str {
		match self {
			Self::Preset(p) => p.emoji(),
			Self::Custom(_) => "📝",
		}
	}
	/// Get the system message; the message that is meant to instruct GPT about what to do.
	pub fn system_message(&self) -> &str {
		match self {
			Self::Preset(p) => p.system_message(),
			Self::Custom(m) => m,
		}
	}
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct PersonalityPreset {
	name: String,
	emoji: String,
	system_message: String,
}

impl PersonalityPreset {
	/// Get a descriptor for the personality for use in messages that say what is set.
	pub fn name(&self) -> &str {
		&self.name
	}
	/// Get the emoji that the bot will use to convey the used preset.
	pub fn emoji(&self) -> &str {
		&self.emoji
	}
	/// Get the system message; the message that is meant to instruct GPT about what to do.
	pub fn system_message(&self) -> &str {
		&self.system_message
	}
}

/// If the string is like `"custom(whatever)"`, returns `Some("whatever")`, otherwise `None`.
///
/// This is the database format for storing a custom system message.
pub fn extract_custom(string: &str) -> Option<&str> {
	string
		.strip_prefix("custom(")
		.and_then(|rem| rem.strip_suffix(")"))
}

/// Makes `"whatever"` into `"custom(whatever)"`.
///
/// I am only doing this here so it's near the other function that encodes the database format for custom system messages.
pub fn wrap_custom(message: &str) -> String {
	format!("custom({message})")
}
