use regex::Regex;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json;
use std::str::FromStr;
use std::sync::LazyLock;
use tracing::debug;

static TIME_VALUE_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^([+-])?(\d+)([+-])?$").expect("Invalid regex for time value parsing")
});

#[derive(Debug, PartialEq, Clone)]
pub enum TimeValue {
    Set(u16),
    Add(i16),
    Subtract(i16),
}

impl FromStr for TimeValue {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let captures = TIME_VALUE_REGEX
            .captures(s)
            .ok_or_else(|| format!("Invalid time value format: {s}"))?;

        let number_str = captures.get(2).unwrap().as_str();
        let number: u16 = number_str
            .parse()
            .map_err(|_| format!("Invalid number: {number_str}"))?;

        // Check for prefix and suffix
        let prefix = captures.get(1).map(|m| m.as_str());
        let suffix = captures.get(3).map(|m| m.as_str());

        if prefix.is_some() && suffix.is_some() {
            return Err(format!("Invalid time value format {s}"));
        }

        match prefix.or(suffix) {
            Some("+") => Ok(TimeValue::Add(number as i16)),
            Some("-") => Ok(TimeValue::Subtract(number as i16)),
            None => Ok(TimeValue::Set(number)),
            // This shouldn't happen with our regex, but just in case
            _ => Err(format!("Invalid time value format: {s}")),
        }
    }
}

impl Serialize for TimeValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            TimeValue::Set(v) => serializer.serialize_str(&v.to_string()),
            TimeValue::Add(v) => serializer.serialize_str(&format!("+{v}")),
            TimeValue::Subtract(v) => serializer.serialize_str(&format!("-{v}")),
        }
    }
}

impl<'de> Deserialize<'de> for TimeValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        TimeValue::from_str(&s).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Message {
    // Simple commands
    Start,
    Stop,
    Toggle,
    Reset,
    NextState,
    // Duration commands
    SetWork { time: TimeValue },
    SetShort { time: TimeValue },
    SetLong { time: TimeValue },
    SetCurrent { time: TimeValue },
}

impl Message {
    pub fn decode(input: &str) -> Result<Self, serde_json::Error> {
        // First try to parse as-is
        match serde_json::from_str(input) {
            Ok(msg) => Ok(msg),
            Err(first_error) => {
                // If it fails, try wrapping in quotes (for simple commands like "start", allow
                // trailing whitespace (like \n)
                let quoted = format!("\"{}\"", input.trim());
                debug!(quoted, "Trying again");
                serde_json::from_str(&quoted).map_err(|_| first_error)
            }
        }
    }

    pub fn encode(&self) -> String {
        serde_json::to_string(self).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_value_from_str() {
        // Test absolute values
        assert_eq!(TimeValue::from_str("25").unwrap(), TimeValue::Set(25));
        assert_eq!(TimeValue::from_str("0").unwrap(), TimeValue::Set(0));
        assert_eq!(TimeValue::from_str("999").unwrap(), TimeValue::Set(999));

        // Test prefix notation
        assert_eq!(TimeValue::from_str("+5").unwrap(), TimeValue::Add(5));
        assert_eq!(TimeValue::from_str("-3").unwrap(), TimeValue::Subtract(3));

        // Test suffix notation
        assert_eq!(TimeValue::from_str("5+").unwrap(), TimeValue::Add(5));
        assert_eq!(TimeValue::from_str("3-").unwrap(), TimeValue::Subtract(3));

        // Test errors
        assert!(TimeValue::from_str("").is_err());
        assert!(TimeValue::from_str("abc").is_err());
        assert!(TimeValue::from_str("+").is_err());
        assert!(TimeValue::from_str("-").is_err());
        assert!(TimeValue::from_str("+-5").is_err());
        assert!(TimeValue::from_str("-+5").is_err());
        assert!(TimeValue::from_str("+abc").is_err());
        assert!(TimeValue::from_str("5+-").is_err());
        assert!(TimeValue::from_str("+5+").is_err());
        assert!(TimeValue::from_str("-5-").is_err());
        assert!(TimeValue::from_str("++5").is_err());
        assert!(TimeValue::from_str("--5").is_err());
    }

    #[test]
    fn test_encode_set_work() {
        let message = Message::SetWork {
            time: TimeValue::Set(25),
        };
        assert_eq!(message.encode(), r#"{"set-work":{"time":"25"}}"#);
    }

    #[test]
    fn test_encode_delta() {
        let message = Message::SetWork {
            time: TimeValue::Add(5),
        };
        assert_eq!(message.encode(), r#"{"set-work":{"time":"+5"}}"#);

        let message = Message::SetWork {
            time: TimeValue::Subtract(5),
        };
        assert_eq!(message.encode(), r#"{"set-work":{"time":"-5"}}"#);
    }

    #[test]
    fn test_decode_set_work() {
        let input = r#"{"set-work":{"time":"25"}}"#;
        let result = Message::decode(input);
        assert!(result.is_ok());
        let message = result.unwrap();
        assert_eq!(
            message,
            Message::SetWork {
                time: TimeValue::Set(25)
            }
        );
    }

    #[test]
    fn test_decode_positive_delta() {
        let input = r#"{"set-work":{"time":"+5"}}"#;
        let result = Message::decode(input);
        assert!(result.is_ok());
        let message = result.unwrap();
        assert_eq!(
            message,
            Message::SetWork {
                time: TimeValue::Add(5)
            }
        );
    }

    #[test]
    fn test_decode_negative_delta() {
        let input = r#"{"set-work":{"time":"-5"}}"#;
        let result = Message::decode(input);
        assert!(result.is_ok());
        let message = result.unwrap();
        assert_eq!(
            message,
            Message::SetWork {
                time: TimeValue::Subtract(5)
            }
        );
    }

    #[test]
    fn test_decode_backward_compat() {
        // Test that plain strings are accepted for simple commands
        assert_eq!(Message::decode("start").unwrap(), Message::Start);
        assert_eq!(Message::decode("stop").unwrap(), Message::Stop);
        assert_eq!(Message::decode("toggle").unwrap(), Message::Toggle);
        assert_eq!(Message::decode("reset").unwrap(), Message::Reset);
        assert_eq!(Message::decode("next-state").unwrap(), Message::NextState);

        // Test with trailing whitespace (like from echo)
        assert_eq!(Message::decode("start\n").unwrap(), Message::Start);
        assert_eq!(Message::decode("stop\n").unwrap(), Message::Stop);
        assert_eq!(Message::decode("toggle\n").unwrap(), Message::Toggle);
        assert_eq!(Message::decode("reset\n").unwrap(), Message::Reset);
        assert_eq!(Message::decode("next-state\n").unwrap(), Message::NextState);
        assert_eq!(Message::decode("  start  \n").unwrap(), Message::Start);

        // Invalid commands should still fail
        assert!(Message::decode("invalid").is_err());
        assert!(Message::decode("invalid\n").is_err());
    }

    #[test]
    fn test_decode_failure_invalid_json() {
        let input = "not json";
        let result = Message::decode(input);
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_failure_empty() {
        let input = "";
        let result = Message::decode(input);
        assert!(result.is_err());
    }

    #[test]
    fn test_encode_simple_commands() {
        assert_eq!(Message::Start.encode(), r#""start""#);
        assert_eq!(Message::Stop.encode(), r#""stop""#);
        assert_eq!(Message::Toggle.encode(), r#""toggle""#);
        assert_eq!(Message::Reset.encode(), r#""reset""#);
        assert_eq!(Message::NextState.encode(), r#""next-state""#);
    }

    #[test]
    fn test_decode_string_values_prefix() {
        // Test prefix notation (+5, -5)
        let input = r#"{"set-work":{"time":"+5"}}"#;
        let result = Message::decode(input);
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            Message::SetWork {
                time: TimeValue::Add(5)
            }
        );

        let input = r#"{"set-work":{"time":"-3"}}"#;
        let result = Message::decode(input);
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            Message::SetWork {
                time: TimeValue::Subtract(3)
            }
        );

        let input = r#"{"set-current":{"time":"+10"}}"#;
        let result = Message::decode(input);
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            Message::SetCurrent {
                time: TimeValue::Add(10)
            }
        );
    }

    #[test]
    fn test_decode_string_values_suffix() {
        // Test suffix notation (5+, 3-)
        let input = r#"{"set-work":{"time":"5+"}}"#;
        let result = Message::decode(input);
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            Message::SetWork {
                time: TimeValue::Add(5)
            }
        );

        let input = r#"{"set-short":{"time":"3-"}}"#;
        let result = Message::decode(input);
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            Message::SetShort {
                time: TimeValue::Subtract(3)
            }
        );
    }

    #[test]
    fn test_decode_string_values_absolute() {
        // Test plain number strings
        let input = r#"{"set-work":{"time":"25"}}"#;
        let result = Message::decode(input);
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            Message::SetWork {
                time: TimeValue::Set(25)
            }
        );

        let input = r#"{"set-long":{"time":"15"}}"#;
        let result = Message::decode(input);
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            Message::SetLong {
                time: TimeValue::Set(15)
            }
        );
    }

    #[test]
    fn test_serde_roundtrip() {
        let messages = vec![
            Message::Start,
            Message::Stop,
            Message::Toggle,
            Message::Reset,
            Message::NextState,
            Message::SetWork {
                time: TimeValue::Set(25),
            },
            Message::SetShort {
                time: TimeValue::Set(5),
            },
            Message::SetLong {
                time: TimeValue::Set(15),
            },
            Message::SetWork {
                time: TimeValue::Add(5),
            },
            Message::SetWork {
                time: TimeValue::Subtract(5),
            },
            Message::SetCurrent {
                time: TimeValue::Set(30),
            },
            Message::SetCurrent {
                time: TimeValue::Add(5),
            },
        ];

        for msg in messages {
            let encoded = msg.encode();
            let decoded = Message::decode(&encoded).unwrap();
            assert_eq!(msg, decoded);
        }
    }
}
