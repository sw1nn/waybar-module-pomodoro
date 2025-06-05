use crate::models::message::{Message, TimeValue};
use crate::services::timer::CycleType;
use clap::{Parser, Subcommand};

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
        value: TimeValue,
    },
    /// Set new short break time [supports: 5, 2+, 1-]
    SetShort {
        value: TimeValue,
    },
    /// Set new long break time [supports: 15, 5+, 2-]
    SetLong {
        value: TimeValue,
    },
    /// Set duration for current timer state [supports: 25, 5+, 3-]
    SetCurrent {
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
    match cycle_type {
        Some(CycleType::Work) => Message::SetWork { time: value.clone() },
        Some(CycleType::ShortBreak) => Message::SetShort { time: value.clone() },
        Some(CycleType::LongBreak) => Message::SetLong { time: value.clone() },
        None => Message::SetCurrent { time: value.clone() },
    }
}
