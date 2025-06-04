use serde::{Deserialize, Serialize};
use serde_json;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub enum Message {
    SetWork { value: i16, is_delta: bool },
    SetShort { value: i16, is_delta: bool },
    SetLong { value: i16, is_delta: bool },
    SetCurrent { value: i16, is_delta: bool },
}

impl Message {
    pub fn new(name: &str, value: i32) -> Self {
        match name {
            "set-work" => Message::SetWork {
                value: value as i16,
                is_delta: false,
            },
            "set-short" => Message::SetShort {
                value: value as i16,
                is_delta: false,
            },
            "set-long" => Message::SetLong {
                value: value as i16,
                is_delta: false,
            },
            "set-current" => Message::SetCurrent {
                value: value as i16,
                is_delta: false,
            },
            _ => panic!("Unknown message type: {}", name),
        }
    }

    pub fn decode(input: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(input)
    }

    pub fn encode(&self) -> String {
        serde_json::to_string(self).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let message = Message::new("set-work", 42);
        assert_eq!(
            message,
            Message::SetWork {
                value: 42,
                is_delta: false
            }
        );
    }

    #[test]
    fn test_encode_set_work() {
        let message = Message::SetWork {
            value: 25,
            is_delta: false,
        };
        assert_eq!(message.encode(), r#"{"SetWork":{"value":25,"is_delta":false}}"#);
    }

    #[test]
    fn test_encode_delta() {
        let message = Message::SetWork {
            value: 5,
            is_delta: true,
        };
        assert_eq!(message.encode(), r#"{"SetWork":{"value":5,"is_delta":true}}"#);

        let message = Message::SetWork {
            value: -5,
            is_delta: true,
        };
        assert_eq!(message.encode(), r#"{"SetWork":{"value":-5,"is_delta":true}}"#);
    }

    #[test]
    fn test_decode_set_work() {
        let input = r#"{"SetWork":{"value":25,"is_delta":false}}"#;
        let result = Message::decode(input);
        assert!(result.is_ok());
        let message = result.unwrap();
        assert_eq!(
            message,
            Message::SetWork {
                value: 25,
                is_delta: false
            }
        );
    }

    #[test]
    fn test_decode_positive_delta() {
        let input = r#"{"SetWork":{"value":5,"is_delta":true}}"#;
        let result = Message::decode(input);
        assert!(result.is_ok());
        let message = result.unwrap();
        assert_eq!(
            message,
            Message::SetWork {
                value: 5,
                is_delta: true
            }
        );
    }

    #[test]
    fn test_decode_negative_delta() {
        let input = r#"{"SetWork":{"value":-5,"is_delta":true}}"#;
        let result = Message::decode(input);
        assert!(result.is_ok());
        let message = result.unwrap();
        assert_eq!(
            message,
            Message::SetWork {
                value: -5,
                is_delta: true
            }
        );
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
    fn test_serde_roundtrip() {
        let messages = vec![
            Message::SetWork { value: 25, is_delta: false },
            Message::SetShort { value: 5, is_delta: false },
            Message::SetLong { value: 15, is_delta: false },
            Message::SetWork { value: 5, is_delta: true },
            Message::SetWork { value: -5, is_delta: true },
            Message::SetCurrent { value: 30, is_delta: false },
            Message::SetCurrent { value: 5, is_delta: true },
        ];
        
        for msg in messages {
            let encoded = msg.encode();
            let decoded = Message::decode(&encoded).unwrap();
            assert_eq!(msg, decoded);
        }
    }
}
