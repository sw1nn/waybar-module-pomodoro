use crate::models::message::Message;
use crate::services::timer::CycleType;
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
    /// Target a specific instance number (e.g., 0, 1, 2)
    #[arg(short = 'i', long = "instance", value_name = "NUM")]
    pub instance: Option<u16>,

    #[command(subcommand)]
    pub operation: Operation,
}

#[derive(Subcommand, Clone)]
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
    /// Set duration for current timer state [supports: 25, 5+, 3-]
    SetCurrent {
        #[arg(value_parser = parse_time_value)]
        value: TimeValue,
    },
    /// Move to the next state (skip current timer)
    NextState,
}

impl Operation {
    pub fn to_message(&self) -> Message {
        match self {
            Operation::Toggle => Message::Toggle,
            Operation::Start => Message::Start,
            Operation::Stop => Message::Stop,
            Operation::Reset => Message::Reset,
            Operation::SetWork { value } => time_value_to_message(value, Some(CycleType::Work)),
            Operation::SetShort { value } => time_value_to_message(value, Some(CycleType::ShortBreak)),
            Operation::SetLong { value } => time_value_to_message(value, Some(CycleType::LongBreak)),
            Operation::SetCurrent { value } => time_value_to_message(value, None),
            Operation::NextState => Message::NextState,
        }
    }
}

fn time_value_to_message(value: &TimeValue, cycle_type: Option<CycleType>) -> Message {
    let (final_value, is_delta) = match value {
        TimeValue::Set(minutes) => (*minutes as i16, false),
        TimeValue::Add(delta) => (*delta, true),
        TimeValue::Subtract(delta) => (-*delta, true),
    };

    match cycle_type {
        Some(CycleType::Work) => Message::SetWork { value: final_value, is_delta },
        Some(CycleType::ShortBreak) => Message::SetShort { value: final_value, is_delta },
        Some(CycleType::LongBreak) => Message::SetLong { value: final_value, is_delta },
        None => Message::SetCurrent { value: final_value, is_delta },
    }
}
