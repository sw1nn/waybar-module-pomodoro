use std::{
    fs,
    io::{BufReader, Error, Read, Write},
    os::unix::net::{UnixListener, UnixStream},
    path::{Path, PathBuf},
    sync::mpsc::{Receiver, Sender},
    thread,
};

use notify_rust::Notification;
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

fn process_message(state: &mut Timer, message: &str) {
    debug!("process_message called with: '{}'", message);
    if let Ok(msg) = Message::decode(message) {
        debug!("Decoded message: {:?}", msg);
        match msg {
            Message::SetWork(value) => state.set_time(CycleType::Work, value),
            Message::SetShort(value) => state.set_time(CycleType::ShortBreak, value),
            Message::SetLong(value) => state.set_time(CycleType::LongBreak, value),
            Message::AddDeltaWork(delta) => state.add_delta_time(CycleType::Work, delta),
            Message::AddDeltaShort(delta) => state.add_delta_time(CycleType::ShortBreak, delta),
            Message::AddDeltaLong(delta) => state.add_delta_time(CycleType::LongBreak, delta),
        }
    } else {
        debug!("Message decode failed, trying raw commands");
        match message {
            "start" => {
                debug!("Setting running to true");
                state.running = true;
            }
            "stop" => {
                debug!("Setting running to false");
                state.running = false;
            }
            "toggle" => {
                debug!(
                    "Toggling running state from {} to {}",
                    state.running, !state.running
                );
                state.running = !state.running;
            }
            "reset" => {
                debug!("Resetting timer");
                state.reset();
            }
            _ => {
                debug!("Unknown message: '{}'", message);
            }
        }
    }
}

fn handle_client(rx: Receiver<String>, socket_path: String, config: Config) {
    let socket_nr = socket_path
        .chars()
        .filter_map(|c| c.to_digit(10))
        .fold(0, |acc, digit| acc * 10 + digit) as i32;

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
            process_message(&mut state, &message);
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
                &class,
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

pub fn get_existing_sockets(binary_name: &str) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = vec![];

    // Use XDG runtime directory for socket discovery
    let xdg_dirs = match BaseDirectories::with_prefix(binary_name) {
        Ok(dirs) => dirs,
        Err(e) => {
            warn!("Failed to get XDG base directories: {}", e);
            return files;
        }
    };

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
        process_message(&mut timer, &Message::new("set-work", 30).encode());
        assert_eq!(get_time(&timer, CycleType::Work), 30 * MINUTE);
    }

    #[test]
    fn test_process_message_set_short() {
        let mut timer = create_timer();
        process_message(&mut timer, &Message::new("set-short", 3).encode());
        assert_eq!(get_time(&timer, CycleType::ShortBreak), 3 * MINUTE);
    }

    #[test]
    fn test_process_message_set_long() {
        let mut timer = create_timer();
        process_message(&mut timer, &Message::new("set-long", 10).encode());
        assert_eq!(get_time(&timer, CycleType::LongBreak), 10 * MINUTE);
    }

    #[test]
    fn test_process_message_start() {
        let mut timer = create_timer();
        process_message(&mut timer, "start");
        assert!(timer.running);
    }

    #[test]
    fn test_process_message_stop() {
        let mut timer = create_timer();
        process_message(&mut timer, "stop");
        assert!(!timer.running);
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
}
