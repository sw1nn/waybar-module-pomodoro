use crate::models::message::Message;
use clap::{Parser, Subcommand};

#[derive(Debug, Clone)]
pub enum TimeValue {
    Set(u16),
    Add(i16),
    Subtract(i16),
}

fn parse_time_value(s: &str) -> Result<TimeValue, String> {
    if s.ends_with('+') {
        let delta_str = s.strip_suffix('+').unwrap();
        let delta: i16 = delta_str
            .parse()
            .map_err(|_| format!("Invalid number before +: {}", delta_str))?;
        Ok(TimeValue::Add(delta))
    } else if s.ends_with('-') {
        let delta_str = s.strip_suffix('-').unwrap();
        let delta: i16 = delta_str
            .parse()
            .map_err(|_| format!("Invalid number before -: {}", delta_str))?;
        Ok(TimeValue::Subtract(delta))
    } else {
        let minutes: u16 = s.parse().map_err(|_| format!("Invalid number: {}", s))?;
        Ok(TimeValue::Set(minutes))
    }
}

#[derive(Parser)]
#[command(name = "waybar-module-pomodoro-ctl")]
#[command(about = "Control interface for waybar-module-pomodoro")]
#[command(long_about = None)]
#[command(version)]
pub struct ControlCli {
    #[command(subcommand)]
    pub operation: Operation,
}

#[derive(Subcommand)]
pub enum Operation {
    /// Toggles the timer
    Toggle,
    /// Start the timer
    Start,
    /// Stop the timer
    Stop,
    /// Reset timer to initial state
    Reset,
    /// Set new work time [supports: 25, 5+, 3-]
    SetWork {
        #[arg(value_parser = parse_time_value)]
        value: TimeValue,
    },
    /// Set new short break time [supports: 5, 2+, 1-]
    SetShort {
        #[arg(value_parser = parse_time_value)]
        value: TimeValue,
    },
    /// Set new long break time [supports: 15, 5+, 2-]
    SetLong {
        #[arg(value_parser = parse_time_value)]
        value: TimeValue,
    },
}

impl Operation {
    pub fn to_message(&self) -> Result<Message, String> {
        match self {
            Operation::SetWork { value } => time_value_to_message(value, "work"),
            Operation::SetShort { value } => time_value_to_message(value, "short"),
            Operation::SetLong { value } => time_value_to_message(value, "long"),
            _ => Err("Not a set operation".to_string()),
        }
    }
}

fn time_value_to_message(value: &TimeValue, cycle_type: &str) -> Result<Message, String> {
    match value {
        TimeValue::Set(minutes) => match cycle_type {
            "work" => Ok(Message::SetWork(*minutes)),
            "short" => Ok(Message::SetShort(*minutes)),
            "long" => Ok(Message::SetLong(*minutes)),
            _ => Err(format!("Unknown cycle type: {}", cycle_type)),
        },
        TimeValue::Add(delta) => match cycle_type {
            "work" => Ok(Message::AddDeltaWork(*delta)),
            "short" => Ok(Message::AddDeltaShort(*delta)),
            "long" => Ok(Message::AddDeltaLong(*delta)),
            _ => Err(format!("Unknown cycle type: {}", cycle_type)),
        },
        TimeValue::Subtract(delta) => match cycle_type {
            "work" => Ok(Message::AddDeltaWork(-*delta)),
            "short" => Ok(Message::AddDeltaShort(-*delta)),
            "long" => Ok(Message::AddDeltaLong(-*delta)),
            _ => Err(format!("Unknown cycle type: {}", cycle_type)),
        },
    }
}
