use std::{
    fs,
    io::{BufReader, Error, Read, Write},
    os::unix::net::{UnixListener, UnixStream},
    path::{Path, PathBuf},
    sync::{
        mpsc::{Receiver, Sender},
        LazyLock,
    },
    thread,
};

use notify_rust::Notification;
use regex::Regex;
use rodio::{Decoder, OutputStream, Sink};
use tracing::{debug, info, warn};
use xdg::BaseDirectories;

use crate::{
    models::{config::Config, message::Message},
    utils::{
        self,
        consts::{HOUR, MINUTE, SLEEP_DURATION},
    },
};

use super::{
    cache,
    timer::{CycleType, Timer},
};

// Shared regex for matching socket filenames with trailing numbers
static SOCKET_NUMBER_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^module(\d+)$").unwrap());

pub fn play_sound(file_path: Option<&str>) {
    debug!("play_sound called with file_path: {:?}", file_path);

    // Return early if no sound file is specified
    let file_path = match file_path {
        Some(path) => path,
        None => {
            debug!("Skipping sound playback: no sound file specified");
            return;
        }
    };

    // Check if file exists
    if !Path::new(file_path).exists() {
        warn!("Sound file not found: {}", file_path);
        return;
    }

    debug!("Starting sound playback for: {}", file_path);

    // Spawn a thread for non-blocking audio playback
    let file_path = file_path.to_string();
    thread::spawn(move || match play_audio_file(&file_path) {
        Ok(_) => debug!("Successfully played sound: {}", file_path),
        Err(e) => warn!("Failed to play sound {}: {}", file_path, e),
    });
}

fn play_audio_file(file_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    debug!("play_audio_file: Creating audio output stream");

    // Create audio output stream
    let (_stream, stream_handle) = OutputStream::try_default()?;
    debug!("play_audio_file: Audio output stream created successfully");

    debug!("play_audio_file: Opening file: {}", file_path);

    // Open and decode the audio file
    let file = fs::File::open(file_path)?;
    let buf_reader = BufReader::new(file);

    debug!("play_audio_file: Decoding audio file");
    let source = Decoder::new(buf_reader)?;
    debug!("play_audio_file: Audio file decoded successfully");

    debug!("play_audio_file: Creating audio sink");

    // Create a sink and play the audio
    let sink = Sink::try_new(&stream_handle)?;
    sink.append(source);
    debug!("play_audio_file: Audio appended to sink, starting playback");

    // Wait for playback to finish
    sink.sleep_until_end();
    debug!("play_audio_file: Playback finished");

    Ok(())
}

pub fn send_notification(cycle_type: CycleType, config: &Config) {
    debug!("send_notification called for cycle_type: {:?}", cycle_type);

    // Check if notifications are enabled
    if config.with_notifications {
        if let Err(e) = Notification::new()
            .summary("Pomodoro")
            .body(match cycle_type {
                CycleType::Work => "Time to work!",
                CycleType::ShortBreak => "Time for a short break!",
                CycleType::LongBreak => "Time for a long break!",
            })
            .show()
        {
            warn!("send_notification failed: {}", e);
        }
    } else {
        debug!("Notifications disabled, skipping desktop notification");
    }

    let sound_file = match cycle_type {
        CycleType::Work => config.work_sound.as_deref(),
        CycleType::ShortBreak | CycleType::LongBreak => config.break_sound.as_deref(),
    };

    debug!("send_notification: Using sound file: {:?}", sound_file);
    play_sound(sound_file)
}

fn format_time(elapsed_time: u16, max_time: u16) -> String {
    let time = max_time - elapsed_time;

    let hour = time / HOUR;
    let minute = (time % HOUR) / MINUTE;
    let second = time % MINUTE;

    if hour > 0 {
        return format!("{:02}:{:02}:{:02}", hour, minute, second);
    }

    format!("{:02}:{:02}", minute, second)
}

fn create_message(value: String, tooltip: &str, class: &str) -> String {
    format!(
        "{{\"text\": \"{}\", \"tooltip\": \"{}\", \"class\": \"{}\", \"alt\": \"{}\"}}",
        value, tooltip, class, class
    )
}

fn process_message(state: &mut Timer, message: &str, config: &Config) {
    debug!("process_message called with: '{}'", message);
    
    match Message::decode(message) {
        Ok(msg) => {
            debug!("Decoded message: {:?}", msg);
            match msg {
                // Simple commands
                Message::Start => {
                    debug!("Setting running to true");
                    state.running = true;
                }
                Message::Stop => {
                    debug!("Setting running to false");
                    state.running = false;
                }
                Message::Toggle => {
                    debug!(
                        "Toggling running state from {} to {}",
                        state.running, !state.running
                    );
                    state.running = !state.running;
                }
                Message::Reset => {
                    debug!("Resetting timer");
                    state.reset();
                }
                Message::NextState => {
                    debug!("Moving to next state");
                    state.next_state(config);
                }
                // Duration commands
                Message::SetWork { value, is_delta } => {
                    if is_delta {
                        state.add_delta_time(CycleType::Work, value)
                    } else {
                        state.set_time(CycleType::Work, value as u16)
                    }
                }
                Message::SetShort { value, is_delta } => {
                    if is_delta {
                        state.add_delta_time(CycleType::ShortBreak, value)
                    } else {
                        state.set_time(CycleType::ShortBreak, value as u16)
                    }
                }
                Message::SetLong { value, is_delta } => {
                    if is_delta {
                        state.add_delta_time(CycleType::LongBreak, value)
                    } else {
                        state.set_time(CycleType::LongBreak, value as u16)
                    }
                }
                Message::SetCurrent { value, is_delta } => {
                    if is_delta {
                        state.add_current_delta_time(value)
                    } else {
                        state.set_current_duration(value as u16)
                    }
                }
            }
        }
        Err(e) => {
            debug!("Failed to decode message '{}': {}", message, e);
        }
    }
}

/// Extract socket number from a socket path by looking only at the filename
/// Only matches numbers at the end of the base filename (before extension)
fn extract_socket_number(socket_path: &str) -> i32 {
    std::path::Path::new(socket_path)
        .file_stem() // without extension
        .and_then(|name| name.to_str())
        .and_then(|name| {
            SOCKET_NUMBER_REGEX
                .captures(name)
                .and_then(|caps| caps.get(1))
                .and_then(|m| m.as_str().parse::<i32>().ok())
        })
        .unwrap_or(0)
}

fn handle_client(rx: Receiver<String>, socket_path: String, config: Config) {
    let socket_nr = extract_socket_number(&socket_path);

    let mut state = Timer::new(
        config.work_time,
        config.short_break,
        config.long_break,
        socket_nr,
    );

    if config.persist {
        let _ = cache::restore(&mut state, &config);
    }

    loop {
        if let Ok(message) = rx.try_recv() {
            debug!("Processing message: '{}'", message);
            process_message(&mut state, &message, &config);
        }

        let value = format_time(state.elapsed_time, state.get_current_time());
        let value_prefix = config.get_play_pause_icon(state.running);
        let tooltip = format!(
            "{} pomodoro{} completed this session",
            state.session_completed,
            if state.session_completed > 1 || state.session_completed == 0 {
                "s"
            } else {
                ""
            }
        );
        let class = state.get_class();
        let cycle_icon = config.get_cycle_icon(state.is_break());
        state.update_state(&config);
        println!(
            "{}",
            create_message(
                utils::helper::trim_whitespace(&format!(
                    "{} {} {}",
                    value_prefix, value, cycle_icon
                )),
                tooltip.as_str(),
                class,
            )
        );

        if state.running {
            state.increment_time();
        }

        if config.persist {
            let _ = cache::store(&state);
        }

        std::thread::sleep(SLEEP_DURATION);
    }
}

fn delete_socket(socket_path: &str) {
    if Path::new(&socket_path).exists() {
        fs::remove_file(socket_path).unwrap();
    }
}

pub fn spawn_module(socket_path: &str, config: Config) {
    info!("Creating socket at: {}", socket_path);
    delete_socket(socket_path);

    let listener = UnixListener::bind(socket_path).unwrap();
    info!("Socket bound successfully");
    let (tx, rx): (Sender<String>, Receiver<String>) = std::sync::mpsc::channel();
    {
        let socket_path = socket_path.to_owned();
        thread::spawn(|| handle_client(rx, socket_path, config));
    }

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                // read incoming data
                let mut message = String::new();
                stream
                    .read_to_string(&mut message)
                    .expect("Failed to read UNIX stream");

                debug!("Received message: '{}'", message);

                if message.contains("exit") {
                    info!("Received exit signal, shutting down module");
                    delete_socket(socket_path);
                    break;
                }
                tx.send(message.to_string()).unwrap();
            }
            Err(err) => warn!("Socket error: {}", err),
        }
    }
}

/// Find the next available instance number by looking at existing sockets
pub fn find_next_instance_number(binary_name: &str) -> u16 {
    let sockets = get_existing_sockets(binary_name);

    // If no sockets exist, return 0 for the first instance
    if sockets.is_empty() {
        return 0;
    }

    let max_instance = sockets
        .iter()
        .filter_map(|socket| {
            socket
                .file_stem() // Get filename without extension
                .and_then(|name| name.to_str())
                .and_then(|name| {
                    SOCKET_NUMBER_REGEX
                        .captures(name)
                        .and_then(|caps| caps.get(1))
                        .and_then(|m| m.as_str().parse::<u16>().ok())
                })
        })
        .max()
        .unwrap_or(0);

    // Return N+1, but ensure we don't overflow (though unlikely with u16)
    max_instance.saturating_add(1)
}

pub fn get_existing_sockets(binary_name: &str) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = vec![];

    // Use XDG runtime directory for socket discovery
    let xdg_dirs = BaseDirectories::with_prefix(binary_name);

    debug!("Looking for socket files using XDG list_runtime_files");

    // Use list_runtime_files to get all files in our XDG runtime directory
    let paths = xdg_dirs.list_runtime_files(".");
    for path in paths {
        if let Some(file_name) = path.file_name() {
            if let Some(file_name_str) = file_name.to_str() {
                debug!("Found file: {}", file_name_str);
                // Look for socket files
                if file_name_str.ends_with(".socket") {
                    debug!("Found socket file, adding: {}", path.display());
                    // Canonicalize the path to ensure it's canonical
                    match path.canonicalize() {
                        Ok(canonical_path) => files.push(canonical_path),
                        Err(e) => {
                            warn!("Failed to canonicalize path {}: {}", path.display(), e);
                            // Fallback to the original path if canonicalization fails
                            files.push(path);
                        }
                    }
                }
            }
        }
    }

    debug!("Found {} matching socket files", files.len());
    files
}

pub fn send_message_socket(socket_path: &str, msg: &str) -> Result<(), Error> {
    debug!("Attempting to connect to socket: {}", socket_path);
    debug!("Message to send: '{}'", msg);
    let mut stream = UnixStream::connect(socket_path)?;
    debug!("Connected to socket successfully");
    stream.write_all(msg.as_bytes())?;
    debug!("Message written successfully");
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::utils::consts::{LONG_BREAK_TIME, SHORT_BREAK_TIME, WORK_TIME};

    use super::*;
    use crate::services::module::CycleType;

    fn create_timer() -> Timer {
        Timer::new(WORK_TIME, SHORT_BREAK_TIME, LONG_BREAK_TIME, 0)
    }

    fn get_time(timer: &Timer, cycle: CycleType) -> u16 {
        match cycle {
            CycleType::Work => timer.times[0],
            CycleType::ShortBreak => timer.times[1],
            CycleType::LongBreak => timer.times[2],
        }
    }

    #[test]
    fn test_send_notification_work() {
        let config = Config::default();
        send_notification(CycleType::Work, &config);
    }

    #[test]
    fn test_send_notification_short_break() {
        let config = Config::default();
        send_notification(CycleType::ShortBreak, &config);
    }

    #[test]
    fn test_send_notification_long_break() {
        let config = Config::default();
        send_notification(CycleType::LongBreak, &config);
    }

    #[test]
    fn test_format_time() {
        assert_eq!(format_time(300, 600), "05:00");
        assert_eq!(format_time(59, 60), "00:01");
        assert_eq!(format_time(0, 120), "02:00");
    }

    #[test]
    fn test_create_message() {
        let message = "Pomodoro";
        let tooltip = "Tooltip";
        let class = "Class";

        let result = create_message(message.to_string(), tooltip, class);
        let expected = format!(
            "{{\"text\": \"{}\", \"tooltip\": \"{}\", \"class\": \"{}\", \"alt\": \"{}\"}}",
            message, tooltip, class, class
        );
        assert!(result == expected);
    }

    #[test]
    fn test_process_message_set_work() {
        let mut timer = create_timer();
        let config = Config::default();
        process_message(&mut timer, r#"{"set-work":{"value":30,"is_delta":false}}"#, &config);
        assert_eq!(get_time(&timer, CycleType::Work), 30 * MINUTE);
    }

    #[test]
    fn test_process_message_set_short() {
        let mut timer = create_timer();
        let config = Config::default();
        process_message(&mut timer, r#"{"set-short":{"value":3,"is_delta":false}}"#, &config);
        assert_eq!(get_time(&timer, CycleType::ShortBreak), 3 * MINUTE);
    }

    #[test]
    fn test_process_message_set_long() {
        let mut timer = create_timer();
        let config = Config::default();
        process_message(&mut timer, r#"{"set-long":{"value":10,"is_delta":false}}"#, &config);
        assert_eq!(get_time(&timer, CycleType::LongBreak), 10 * MINUTE);
    }

    #[test]
    fn test_process_message_start() {
        let mut timer = create_timer();
        // Test backward compatibility - plain string should work
        let config = Config::default();
        process_message(&mut timer, "start", &config);
        assert!(timer.running);
    }

    #[test]
    fn test_process_message_stop() {
        let mut timer = create_timer();
        timer.running = true;
        // Test backward compatibility - plain string should work
        let config = Config::default();
        process_message(&mut timer, "stop", &config);
        assert!(!timer.running);
    }

    #[test]
    fn test_process_message_set_current() {
        let mut timer = create_timer();

        // Test setting current work time
        timer.current_index = 0;
        let config = Config::default();
        process_message(&mut timer, r#"{"set-current":{"value":30,"is_delta":false}}"#, &config);
        assert_eq!(timer.times[0], 30 * 60);

        // Test setting current break time
        timer.current_index = 1;
        process_message(&mut timer, r#"{"set-current":{"value":10,"is_delta":false}}"#, &config);
        assert_eq!(timer.times[1], 10 * 60);

        // Test delta on current
        process_message(&mut timer, r#"{"set-current":{"value":5,"is_delta":true}}"#, &config);
        assert_eq!(timer.times[1], 15 * 60);

        // Test negative delta
        process_message(&mut timer, r#"{"set-current":{"value":-2,"is_delta":true}}"#, &config);
        assert_eq!(timer.times[1], 13 * 60);
    }

    // TODO:
    // #[tokio::test]
    // async fn test_spawn_module() {
    // }

    // TODO:
    // #[tokio::test]
    // async fn test_handle_client() {
    // }

    // TODO:
    // #[tokio::test]
    // async fn test_send_message_socket() {
    // }

    #[test]
    fn test_delete_socket() {
        let socket_path = "/tmp/waybar-module-pomodoro_test_socket";
        std::fs::File::create(socket_path).unwrap();
        assert!(std::path::Path::new(socket_path).exists());

        delete_socket(socket_path);
        assert!(!std::path::Path::new(socket_path).exists());
    }

    #[test]
    fn test_find_next_instance_number() {
        // Note: This test is limited because find_next_instance_number uses XDG directories
        // In a real test environment, we'd need to mock the XDG base directories

        // For now, we can at least test the logic by creating a separate test
        // that tests the extraction of numbers from filenames
    }

    #[test]
    fn test_extract_socket_number() {
        // Test with just filename - valid module names
        assert_eq!(extract_socket_number("module0.socket"), 0);
        assert_eq!(extract_socket_number("module1.socket"), 1);
        assert_eq!(extract_socket_number("module123.socket"), 123);

        // Test with full paths
        assert_eq!(
            extract_socket_number("/run/user/1000/waybar-module-pomodoro/module0.socket"),
            0
        );
        assert_eq!(extract_socket_number("/var/tmp/module42.socket"), 42);

        // Test with paths containing numbers
        assert_eq!(
            extract_socket_number("/run/user/1000/waybar-module-pomodoro/module5.socket"),
            5
        );
        assert_eq!(
            extract_socket_number("/home/user123/sockets/module7.socket"),
            7
        );

        // Test edge cases - these should all return 0 because they don't match the pattern
        assert_eq!(extract_socket_number("module.socket"), 0); // No number at end
        assert_eq!(extract_socket_number("custom99name88.socket"), 0); // Not "module" prefix
        assert_eq!(extract_socket_number("99module.socket"), 0); // Wrong pattern
        assert_eq!(extract_socket_number("/path/to/nowhere"), 0); // No extension
        assert_eq!(extract_socket_number(""), 0); // Empty string

        // Test various filenames that don't match the pattern
        assert_eq!(extract_socket_number("socket1.socket"), 0); // Wrong prefix
        assert_eq!(extract_socket_number("my-socket-15.socket"), 0); // Wrong prefix
        assert_eq!(extract_socket_number("test_socket_999.socket"), 0); // Wrong prefix
        assert_eq!(extract_socket_number("modules123.socket"), 0); // Wrong prefix (plural)
        assert_eq!(extract_socket_number("module_123.socket"), 0); // Has underscore
    }
}
