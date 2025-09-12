use std::{collections::HashMap, fs, path::Path};

use reqwest::header::HeaderValue;
use serde::Deserialize;
use serenity::all::{RoleId, UserId};

use crate::{
	allowances::{DEFAULT_ACCRUAL_DAYS, DEFAULT_DAILY_ALLOWANCE},
	gpt::GptModel,
	one_off_response::OneOffCommand,
	response_styles::{extract_custom, PersonalityPreset},
};

#[derive(Debug, Clone)]
pub struct Config {
	pub daily_allowance: u32,
	pub accrual_days: f32,
	pub models: Vec<GptModel>,
	pub search_models: Vec<GptModel>,
	pub personalities: Vec<PersonalityPreset>,
	pub one_offs: Vec<OneOffCommand>,
	pub prototyping_roles: Vec<RoleId>,
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
			models: value.models.expect("There needs to be at least one model."),
			search_models: value.search_models.unwrap_or_default(),
			personalities: value
				.personalities
				.expect("There needs to be at least one personality."),
			one_offs: value.one_offs.unwrap_or_default(),
			prototyping_roles: value.prototyping_roles.unwrap_or_default(),
		};
		if config.models.is_empty() {
			panic!("There needs to be at least one model.");
		}
		if config.personalities.is_empty() {
			panic!("There needs to be at least one personality.");
		}
		if config
			.personalities
			.iter()
			.any(|p| extract_custom(p.name()).is_some())
		{
			panic!("Don't name any personality \"custom(whatever)\".");
		}
		config
	}
}

#[derive(Deserialize)]
struct PartialConfig {
	daily_allowance: Option<u32>,
	accrual_days: Option<f32>,
	models: Option<Vec<GptModel>>,
	search_models: Option<Vec<GptModel>>,
	personalities: Option<Vec<PersonalityPreset>>,
	one_offs: Option<Vec<OneOffCommand>>,
	prototyping_roles: Option<Vec<RoleId>>,
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
