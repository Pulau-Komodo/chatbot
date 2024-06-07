use std::{collections::HashMap, fs, path::Path};

use reqwest::header::HeaderValue;
use serde::Deserialize;
use serenity::all::UserId;

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

pub struct CustomApiKeys(HashMap<UserId, String>);

impl CustomApiKeys {
	pub fn from_file<P: AsRef<Path>>(path: P) -> Self {
		let keys = CustomApiKeysFile::from_file(path);
		let map = keys
			.keys
			.into_iter()
			.map(|key_entry| {
				let user_id = UserId::new(
					key_entry
						.user
						.parse()
						.expect("Fail to parse user ID in custom tokens file."),
				);
				(user_id, key_entry.key)
			})
			.collect();
		Self(map)
	}
	pub fn into_headers(self) -> HashMap<UserId, HeaderValue> {
		self.0
			.into_iter()
			.map(|(user_id, api_key)| {
				let header =
					HeaderValue::from_bytes(format!("Bearer {api_key}").as_bytes()).unwrap();
				(user_id, header)
			})
			.collect()
	}
}

#[derive(Deserialize)]
struct CustomApiKeyEntry {
	user: String,
	key: String,
}

#[derive(Deserialize)]
struct CustomApiKeysFile {
	keys: Vec<CustomApiKeyEntry>,
}

impl CustomApiKeysFile {
	fn from_file<P: AsRef<Path>>(path: P) -> Self {
		toml::from_str(&fs::read_to_string(path).expect("Failed to read custom tokens file."))
			.expect("Failed to parse custom tokens file.")
	}
}
