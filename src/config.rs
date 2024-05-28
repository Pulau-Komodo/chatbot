use std::{fs, path::Path};

use serde::Deserialize;

use crate::{
	allowances::{DEFAULT_ACCRUAL_DAYS, DEFAULT_DAILY_ALLOWANCE},
	chatgpt::ChatgptModel,
	one_off_response::OneOffCommand,
	response_styles::Personality,
};

#[derive(Debug, Clone)]
pub struct Config {
	pub daily_allowance: u32,
	pub accrual_days: f32,
	pub default_model: ChatgptModel,
	pub models: Vec<ChatgptModel>,
	pub personalities: Vec<Personality>,
	pub one_offs: Vec<OneOffCommand>,
}

impl Config {
	pub fn from_file<P: AsRef<Path>>(path: P) -> Self {
		PartialConfig::from_file(path).into()
	}
}

impl From<PartialConfig> for Config {
	fn from(value: PartialConfig) -> Self {
		let config = Self {
			daily_allowance: value.daily_allowance.unwrap_or(DEFAULT_DAILY_ALLOWANCE),
			accrual_days: value.accrual_days.unwrap_or(DEFAULT_ACCRUAL_DAYS),
			default_model: value
				.default_model
				.expect("Default model was not specified in config."),
			models: value.models.unwrap_or_default(),
			personalities: value
				.personalities
				.expect("There needs to be at least one personality."),
			one_offs: value.one_offs.unwrap_or_default(),
		};
		if config.personalities.is_empty() {
			panic!("There needs to be at least one personality.");
		}
		config
	}
}

#[derive(Deserialize)]
struct PartialConfig {
	daily_allowance: Option<u32>,
	accrual_days: Option<f32>,
	default_model: Option<ChatgptModel>,
	models: Option<Vec<ChatgptModel>>,
	personalities: Option<Vec<Personality>>,
	one_offs: Option<Vec<OneOffCommand>>,
}

impl PartialConfig {
	fn from_file<P: AsRef<Path>>(path: P) -> Self {
		toml::from_str(&fs::read_to_string(path).expect("Failed to read config file."))
			.expect("Failed to parse config file.")
	}
}
