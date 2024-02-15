use std::{fs, path::Path};

use serde::Deserialize;

use crate::{
	allowances::{DEFAULT_ACCRUAL_DAYS, DEFAULT_DAILY_ALLOWANCE},
	chatgpt::ChatgptModel,
	response_styles::Personality,
};

#[derive(Debug, Clone)]
pub struct Config {
	pub daily_allowance: u32,
	pub accrual_days: f32,
	pub default_model: ChatgptModel,
	pub fancy_model: ChatgptModel,
}

impl Config {
	pub fn from_file<P: AsRef<Path>>(path: P) -> Self {
		PartialConfig::from_file(path).into()
	}
}

impl From<PartialConfig> for Config {
	fn from(value: PartialConfig) -> Self {
		Self {
			daily_allowance: value.daily_allowance.unwrap_or(DEFAULT_DAILY_ALLOWANCE),
			accrual_days: value.accrual_days.unwrap_or(DEFAULT_ACCRUAL_DAYS),
			default_model: value
				.default_model
				.expect("Default model was not specified in config."),
			fancy_model: value
				.fancy_model
				.expect("Fancy model was not specified in config."),
		}
	}
}

#[derive(Deserialize)]
struct PartialConfig {
	daily_allowance: Option<u32>,
	accrual_days: Option<f32>,
	default_model: Option<ChatgptModel>,
	fancy_model: Option<ChatgptModel>,
}

impl PartialConfig {
	fn from_file<P: AsRef<Path>>(path: P) -> Self {
		toml::from_str(&fs::read_to_string(path).expect("Failed to read config file."))
			.expect("Failed to parse config file.")
	}
}

/// Stores all the system messages used by the application.
#[derive(Deserialize)]
pub struct SystemMessages {
	robotic: String,
	friendly: String,
	poetic: String,
	villainous: String,
	pub dictionary: String,
	pub judgment: String,
}

impl SystemMessages {
	pub fn from_file<P: AsRef<Path>>(path: P) -> Self {
		toml::from_str(&fs::read_to_string(path).expect("Failed to read system messages file."))
			.expect("Failed to parse system messages file.")
	}
	/// Retrieve the system message used for a specific personality.
	pub fn personality_message<'s>(&'s self, personality: &'s Personality) -> &'s str {
		match personality {
			Personality::Robotic => &self.robotic,
			Personality::Friendly => &self.friendly,
			Personality::Poetic => &self.poetic,
			Personality::Villainous => &self.villainous,
			Personality::Custom(text) => text,
		}
	}
}
