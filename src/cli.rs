use crate::utils::consts::{
    BREAK_ICON, LONG_BREAK_TIME, MINUTE, PAUSE_ICON, PLAY_ICON, SHORT_BREAK_TIME, WORK_ICON,
    WORK_TIME,
};
use clap::Parser;
use std::env;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum LogOption {
    Journald,
    File { path: PathBuf },
}

impl std::str::FromStr for LogOption {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "journald" || s.is_empty() {
            Ok(LogOption::Journald)
        } else {
            // Validate the file path
            let path_buf = PathBuf::from(s);

            // Get the parent directory, defaulting to current directory if none specified
            let parent_dir = match path_buf.parent() {
                Some(dir) if !dir.as_os_str().is_empty() => dir.to_path_buf(),
                _ => env::current_dir()
                    .map_err(|e| format!("Cannot get current directory: {}", e))?,
            };

            // Check if parent directory exists
            if !parent_dir.exists() {
                return Err(format!(
                    "Directory does not exist: {}",
                    parent_dir.display()
                ));
            }

            // Check if parent directory is writable
            match fs::metadata(&parent_dir) {
                Ok(metadata) => {
                    if metadata.permissions().readonly() {
                        return Err(format!(
                            "Directory is not writable: {}",
                            parent_dir.display()
                        ));
                    }
                }
                Err(e) => {
                    return Err(format!(
                        "Cannot access directory {}: {}",
                        parent_dir.display(),
                        e
                    ));
                }
            }

            // Return the full path
            let full_path = if path_buf.parent().is_none()
                || path_buf.parent().unwrap().as_os_str().is_empty()
            {
                parent_dir.join(&path_buf)
            } else {
                path_buf
            };

            Ok(LogOption::File { path: full_path })
        }
    }
}

fn validate_sound_file_path(path: &str) -> Result<String, String> {
    let path_buf = PathBuf::from(path);

    // Check if file exists
    if !path_buf.exists() {
        return Err(format!("Sound file does not exist: {}", path));
    }

    // Check if it's a file (not a directory)
    if !path_buf.is_file() {
        return Err(format!("Path is not a file: {}", path));
    }

    // Check if file is readable
    match fs::File::open(&path_buf) {
        Ok(_) => Ok(path.to_string()),
        Err(e) => Err(format!("Cannot read sound file {}: {}", path, e)),
    }
}

#[derive(Parser, Debug)]
#[command(name = "waybar-module-pomodoro")]
#[command(about = "A pomodoro timer module for your system bar")]
#[command(long_about = None)]
#[command(version)]
pub struct ModuleCli {
    /// Sets how long a work cycle is, in minutes
    #[arg(short = 'w', long = "work", value_name = "value", help = format!("Sets how long a work cycle is, in minutes. default: {}", WORK_TIME / MINUTE))]
    pub work: Option<u16>,

    /// Sets how long a short break is, in minutes
    #[arg(short = 's', long = "shortbreak", value_name = "value", help = format!("Sets how long a short break is, in minutes. default: {}", SHORT_BREAK_TIME / MINUTE))]
    pub shortbreak: Option<u16>,

    /// Sets how long a long break is, in minutes
    #[arg(short = 'l', long = "longbreak", value_name = "value", help = format!("Sets how long a long break is, in minutes. default: {}", LONG_BREAK_TIME / MINUTE))]
    pub longbreak: Option<u16>,

    /// Sets custom play icon/text
    #[arg(short = 'p', long = "play", value_name = "value", help = format!("Sets custom play icon/text. default: {}", PLAY_ICON))]
    pub play: Option<String>,

    /// Sets custom pause icon/text
    #[arg(short = 'a', long = "pause", value_name = "value", help = format!("Sets custom pause icon/text. default: {}", PAUSE_ICON))]
    pub pause: Option<String>,

    /// Sets custom work icon/text
    #[arg(short = 'o', long = "work-icon", value_name = "value", help = format!("Sets custom work icon/text. default: {}", WORK_ICON))]
    pub work_icon: Option<String>,

    /// Sets custom break icon/text
    #[arg(short = 'b', long = "break-icon", value_name = "value", help = format!("Sets custom break icon/text. default: {}", BREAK_ICON))]
    pub break_icon: Option<String>,

    /// Sound to play at the end of a work period
    #[arg(
        short = 'O',
        long = "work-sound",
        value_name = "value",
        value_parser = validate_sound_file_path,
        help = "Sound to play at the end of a work period. Omit for silence."
    )]
    pub work_sound: Option<String>,

    /// Sound to play at the end of a break period
    #[arg(
        short = 'B',
        long = "break-sound",
        value_name = "value",
        value_parser = validate_sound_file_path,
        help = "Sound to play at the end of a break period. Omit for silence."
    )]
    pub break_sound: Option<String>,

    /// Disable the pause/play icon
    #[arg(long = "no-icons", help = "Disable the pause/play icon")]
    pub no_icons: bool,

    /// Disable the work/break icon
    #[arg(long = "no-work-icons", help = "Disable the work/break icon")]
    pub no_work_icons: bool,

    /// Starts a work cycle automatically after a break
    #[arg(
        long = "autow",
        help = "Starts a work cycle automatically after a break"
    )]
    pub autow: bool,

    /// Starts a break cycle automatically after work
    #[arg(long = "autob", help = "Starts a break cycle automatically after work")]
    pub autob: bool,

    /// Persist timer state between sessions
    #[arg(long = "persist", help = "Persist timer state between sessions")]
    pub persist: bool,

    /// Enable desktop notifications
    #[arg(long = "with-notifications", help = "Enable desktop notifications")]
    pub with_notifications: bool,

    /// Enable logging to file or journald
    #[arg(long = "log", value_name = "destination", num_args = 0..=1, default_missing_value = "journald", help = "Enable logging. Optionally specify a log file path. If no path is provided, logs to journald")]
    pub log: Option<LogOption>,

    /// Specify instance number (defaults to next available)
    #[arg(short = 'i', long = "instance", value_name = "NUM")]
    pub instance: Option<u16>,
}
