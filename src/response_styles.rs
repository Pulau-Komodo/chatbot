use std::borrow::Cow;

#[derive(Default, Clone, PartialEq, Eq)]
pub enum Personality {
	#[default]
	Robotic,
	Friendly,
	Poetic,
	Villainous,
	Custom(String),
}

impl Personality {
	/// Construct a `Personality` from the way the message is stored in the datase.
	pub fn from_database_str(str: &str) -> Self {
		match str {
			"robotic" => Self::Robotic,
			"friendly" => Self::Friendly,
			"poetic" => Self::Poetic,
			"villainous" => Self::Villainous,
			_ => {
				let custom = str
					.strip_prefix("custom: ")
					.unwrap_or_else(|| panic!("There is no predefined system message {str}"));
				Self::Custom(String::from(custom))
			}
		}
	}
	/// Output as a string the way the message is stored in the database.
	pub fn to_database_string(&self) -> Cow<'static, str> {
		match self {
			Self::Robotic => Cow::from("robotic"),
			Self::Friendly => Cow::from("friendly"),
			Self::Poetic => Cow::from("poetic"),
			Self::Villainous => Cow::from("villainous"),
			Self::Custom(text) => Cow::from(format!("custom: {text}")),
		}
	}
	/// Get the emoji that the bot will use to convey the used preset.
	pub fn emoji(&self) -> &'static str {
		match self {
			Self::Robotic => "🖥️",
			Self::Friendly => "🙂",
			Self::Poetic => "🧑‍🎨",
			Self::Villainous => "🦹‍♂️",
			Self::Custom(_) => "💬",
		}
	}
	/// Get a descriptor for the system message for use in messages that say what is set.
	pub fn name(&self) -> &'static str {
		match self {
			Self::Robotic => "robotic",
			Self::Friendly => "friendly",
			Self::Poetic => "poetic",
			Self::Villainous => "villainous",
			Self::Custom(_) => "a custom message",
		}
	}
}
