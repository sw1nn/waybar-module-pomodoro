use clap::Parser;
use cli::ModuleCli;
use models::config::Config;
use services::module::{send_message_socket, spawn_module};
use signal_hook::{
    consts::{SIGHUP, SIGINT, SIGTERM},
    iterator::Signals,
};
use std::thread;
use tracing::info;
use tracing_subscriber::EnvFilter;
use xdg::BaseDirectories;

mod cli;
mod models;
mod services;
mod utils;

fn setup_tracing(log_file: Option<std::path::PathBuf>) {
    // Server: log to file
    let log_path = log_file.unwrap_or_else(|| std::path::PathBuf::from("/tmp/waybar-pomodoro.log"));

    // Extract directory and filename
    let log_dir = log_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("/tmp"));
    let log_filename = log_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("waybar-pomodoro.log");

    let file_appender = tracing_appender::rolling::daily(log_dir, log_filename);
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::fmt()
        .with_writer(non_blocking)
        .with_env_filter(
            EnvFilter::from_default_env()
                .add_directive("waybar_module_pomodoro=debug".parse().unwrap()),
        )
        .init();

    // Prevent the guard from being dropped
    std::mem::forget(_guard);
}

fn main() -> std::io::Result<()> {
    let cli = ModuleCli::parse();

    setup_tracing(cli.log_file.clone());

    // Debug output of CLI arguments
    tracing::debug!("Parsed CLI arguments: {:#?}", cli);

    let config = Config::from_module_cli(&cli);

    // Use XDG runtime directory for socket
    let xdg_dirs = BaseDirectories::with_prefix("waybar-module-pomodoro")
        .expect("Failed to get XDG base directories");

    let socket_path = xdg_dirs
        .place_runtime_file("module.socket")
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
