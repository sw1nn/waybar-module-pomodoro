use regex::Regex;
use serde::{Deserialize, Serialize};
use std::error::Error;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub enum Message {
    SetWork(u16),
    SetShort(u16),
    SetLong(u16),
    AddDeltaWork(i16),
    AddDeltaShort(i16),
    AddDeltaLong(i16),
}

impl Message {
    pub fn new(name: &str, value: i32) -> Self {
        match name {
            "set-work" => Message::SetWork(value as u16),
            "set-short" => Message::SetShort(value as u16),
            "set-long" => Message::SetLong(value as u16),
            _ => panic!("Unknown message type: {}", name),
        }
    }

    pub fn decode(input: &str) -> Result<Self, Box<dyn Error>> {
        let re = Regex::new(r"\[(.*?)\;(.*?)\]").unwrap();
        match re.captures(input) {
            Some(caps) => {
                let extracted: (&str, [&str; 2]) = caps.extract();
                println!("{:?}", extracted);
                if extracted.1[0].is_empty() {
                    return Err(format!("message name is missing. msg == {:?}", extracted).into());
                }

                let command = extracted.1[0];
                let value_str = extracted.1[1];

                if value_str.starts_with("add:") {
                    let delta: i16 = value_str.strip_prefix("add:").unwrap().parse()?;
                    match command {
                        "set-work" => Ok(Message::AddDeltaWork(delta)),
                        "set-short" => Ok(Message::AddDeltaShort(delta)),
                        "set-long" => Ok(Message::AddDeltaLong(delta)),
                        _ => Err(format!("Unknown command: {}", command).into()),
                    }
                } else if value_str.starts_with("minus:") {
                    let delta: i16 = value_str.strip_prefix("minus:").unwrap().parse()?;
                    match command {
                        "set-work" => Ok(Message::AddDeltaWork(-delta)),
                        "set-short" => Ok(Message::AddDeltaShort(-delta)),
                        "set-long" => Ok(Message::AddDeltaLong(-delta)),
                        _ => Err(format!("Unknown command: {}", command).into()),
                    }
                } else {
                    let value: u16 = value_str.parse()?;
                    match command {
                        "set-work" => Ok(Message::SetWork(value)),
                        "set-short" => Ok(Message::SetShort(value)),
                        "set-long" => Ok(Message::SetLong(value)),
                        _ => Err(format!("Unknown command: {}", command).into()),
                    }
                }
            }
            None => Err(format!("unable to decode message: {input}").into()),
        }
    }

    pub fn encode(&self) -> String {
        match self {
            Message::SetWork(value) => format!("[set-work;{}]", value),
            Message::SetShort(value) => format!("[set-short;{}]", value),
            Message::SetLong(value) => format!("[set-long;{}]", value),
            Message::AddDeltaWork(delta) => {
                if *delta >= 0 {
                    format!("[set-work;add:{}]", delta)
                } else {
                    format!("[set-work;minus:{}]", -delta)
                }
            }
            Message::AddDeltaShort(delta) => {
                if *delta >= 0 {
                    format!("[set-short;add:{}]", delta)
                } else {
                    format!("[set-short;minus:{}]", -delta)
                }
            }
            Message::AddDeltaLong(delta) => {
                if *delta >= 0 {
                    format!("[set-long;add:{}]", delta)
                } else {
                    format!("[set-long;minus:{}]", -delta)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let message = Message::new("set-work", 42);
        assert_eq!(message, Message::SetWork(42));
    }

    #[test]
    fn test_encode_set_work() {
        let message = Message::SetWork(25);
        assert_eq!(message.encode(), "[set-work;25]");
    }

    #[test]
    fn test_encode_add_delta() {
        let message = Message::AddDeltaWork(5);
        assert_eq!(message.encode(), "[set-work;add:5]");

        let message = Message::AddDeltaWork(-5);
        assert_eq!(message.encode(), "[set-work;minus:5]");
    }

    #[test]
    fn test_decode_set_work() {
        let input = "[set-work;25]";
        let result = Message::decode(input);
        assert!(result.is_ok());
        let message = result.unwrap();
        assert_eq!(message, Message::SetWork(25));
    }

    #[test]
    fn test_decode_add_delta() {
        let input = "[set-work;add:5]";
        let result = Message::decode(input);
        assert!(result.is_ok());
        let message = result.unwrap();
        assert_eq!(message, Message::AddDeltaWork(5));
    }

    #[test]
    fn test_decode_minus_delta() {
        let input = "[set-work;minus:5]";
        let result = Message::decode(input);
        assert!(result.is_ok());
        let message = result.unwrap();
        assert_eq!(message, Message::AddDeltaWork(-5));
    }

    #[test]
    fn test_decode_failure_missing_name() {
        let input = "[;7]";
        let result = Message::decode(input);
        assert!(result.is_err());
        if let Err(e) = result {
            assert_eq!(
                e.to_string(),
                "message name is missing. msg == (\"[;7]\", [\"\", \"7\"])"
            );
        }
    }

    #[test]
    fn test_decode_failure_no_input() {
        let input = "";
        let result = Message::decode(input);
        assert!(result.is_err());
        if let Err(e) = result {
            assert_eq!(e.to_string(), "unable to decode message: ");
        }
    }

    #[test]
    fn test_decode_failure_unknown_command() {
        let input = "[unknown;42]";
        let result = Message::decode(input);
        assert!(result.is_err());
        if let Err(e) = result {
            assert_eq!(e.to_string(), "Unknown command: unknown");
        }
    }
}
