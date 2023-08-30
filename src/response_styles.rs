use std::borrow::Cow;

#[derive(Default, Clone, PartialEq, Eq)]
pub enum SystemMessage {
	#[default]
	Robotic,
	Friendly,
	Poetic,
	Villainous,
	Custom(String),
}

impl SystemMessage {
	/// Construct a `SystemMessage` from the way the message is stored in the datase.
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
	/// Get the actual text of the system message for sending to the API.
	pub fn text(&self) -> String {
		match self {
			Self::Robotic => String::from("You are a computer assistant. Reply tersely and robotically."),
			Self::Friendly => String::from("Reply briefly, but in a friendly way."),
			Self::Poetic => String::from("Deliver your answers as short poems. When that is not possible, at least try to insert a lot of rhyme."),
			Self::Villainous => String::from("Answer helpfully, but in a terse, condescending villain speech."),
			Self::Custom(text) => text.clone(),
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
