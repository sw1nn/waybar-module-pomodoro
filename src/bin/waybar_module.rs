use clap::Parser;
use signal_hook::{
    consts::{SIGHUP, SIGINT, SIGTERM},
    iterator::Signals,
};
use std::thread;
use tracing::info;
use tracing_subscriber::EnvFilter;
use waybar_module_pomodoro::cli::{LogOption, ModuleCli};
use waybar_module_pomodoro::models::config::Config;
use waybar_module_pomodoro::services::module::{
    find_next_instance_number, send_message_socket, spawn_module,
};
use xdg::BaseDirectories;

fn setup_tracing(log_option: Option<LogOption>) {
    let env_filter = EnvFilter::from_default_env()
        .add_directive("waybar_module_pomodoro=debug".parse().unwrap());

    match log_option {
        None => {
            // No logging - just return without initializing tracing
        }
        Some(LogOption::Journald) => {
            // Log to journald
            if let Ok(journald_layer) = tracing_journald::layer() {
                use tracing_subscriber::prelude::*;
                tracing_subscriber::registry()
                    .with(env_filter)
                    .with(journald_layer)
                    .init();
            } else {
                eprintln!("Failed to initialize journald logging");
            }
        }
        Some(LogOption::File { path }) => {
            // Log to file
            // Extract directory and filename
            let log_dir = path
                .parent()
                .unwrap_or_else(|| std::path::Path::new("/tmp"));
            let log_filename = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("waybar-pomodoro.log");

            let file_appender = tracing_appender::rolling::daily(log_dir, log_filename);
            let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

            tracing_subscriber::fmt()
                .with_writer(non_blocking)
                .with_env_filter(env_filter)
                .init();

            // Prevent the guard from being dropped
            std::mem::forget(_guard);
        }
    }
}

fn main() -> std::io::Result<()> {
    let cli = ModuleCli::parse();

    setup_tracing(cli.log.clone());

    // Debug output of CLI arguments
    tracing::debug!("Parsed CLI arguments: {:#?}", cli);

    let config = Config::from_module_cli(&cli);

    // Use XDG runtime directory for socket
    let xdg_dirs = BaseDirectories::with_prefix("waybar-module-pomodoro");

    // Determine instance number
    let instance = match cli.instance {
        Some(num) => num,
        None => find_next_instance_number("waybar-module-pomodoro"),
    };

    let socket_filename = format!("module{}.socket", instance);
    let socket_path = xdg_dirs
        .place_runtime_file(&socket_filename)
        .expect("Failed to create socket path in runtime directory")
        .to_string_lossy()
        .to_string();

    info!("Starting module");
    info!("Socket path: {}", socket_path);

    process_signals(socket_path.clone());
    spawn_module(&socket_path, config);

    Ok(())
}

// we need to handle signals to ensure a graceful exit
// this is important because we need to remove the sockets on exit
fn process_signals(socket_path: String) {
    // all possible realtime UNIX signals
    let sigrt = 34..64;

    // intentionally ignore realtime signals
    // if we don't do this, the process will terminate if the user sends SIGRTMIN+N to the bar
    let _dont_handle = Signals::new(sigrt.collect::<Vec<i32>>()).unwrap();

    let mut signals = Signals::new([SIGINT, SIGTERM, SIGHUP]).unwrap();
    thread::spawn(move || {
        for _ in signals.forever() {
            send_message_socket(&socket_path, "exit").expect("unable to send message to module");
        }
    });
}
