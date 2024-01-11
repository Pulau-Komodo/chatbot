use std::{fs, path::Path};

use serde::Deserialize;

use crate::allowances::{DEFAULT_ACCRUAL_DAYS, DEFAULT_DAILY_ALLOWANCE};

#[derive(Clone, Copy)]
pub struct Config {
	pub daily_allowance: u32,
	pub accrual_days: f32,
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
		}
	}
}

#[derive(Deserialize)]
struct PartialConfig {
	daily_allowance: Option<u32>,
	accrual_days: Option<f32>,
}

impl PartialConfig {
	fn from_file<P: AsRef<Path>>(path: P) -> Self {
		toml::from_str(&fs::read_to_string(path).expect("Failed to read config file."))
			.expect("Failed to parse config file.")
	}
}
