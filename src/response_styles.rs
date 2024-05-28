use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Personality {
	name: String,
	emoji: String,
	system_message: String,
}

impl Personality {
	/// Get a descriptor for the system message for use in messages that say what is set.
	pub fn name(&self) -> &str {
		&self.name
	}
	/// Get the emoji that the bot will use to convey the used preset.
	pub fn emoji(&self) -> &str {
		&self.emoji
	}
	pub fn system_message(&self) -> &str {
		&self.system_message
	}
}
