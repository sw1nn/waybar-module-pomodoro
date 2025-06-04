use clap::Parser;
use std::env;
use tracing::{debug, warn};
use tracing_subscriber::EnvFilter;

use waybar_module_pomodoro::control_cli::ControlCli;
use waybar_module_pomodoro::services::module::{get_existing_sockets, send_message_socket};

fn setup_tracing() {
    // Client: log to console, respecting RUST_LOG environment variable
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();
}

fn main() -> std::io::Result<()> {
    let cli = ControlCli::parse();
    setup_tracing();

    let binary_name = env::current_exe()
        .ok()
        .and_then(|path| path.file_name().map(|s| s.to_owned()))
        .and_then(|s| s.to_str().map(|s| s.to_string()))
        .unwrap_or_else(|| "waybar-module-pomodoro".to_string())
        .replace("-ctl", ""); // Remove -ctl to match module socket names

    let mut sockets = get_existing_sockets(&binary_name);
    debug!("Found {} existing sockets", sockets.len());

    // Filter by instance if specified
    if let Some(instance) = cli.instance {
        let target_socket_name = format!("module{}.socket", instance);
        sockets.retain(|socket| {
            socket
                .file_name()
                .and_then(|name| name.to_str())
                .map(|name| name == target_socket_name)
                .unwrap_or(false)
        });

        if sockets.is_empty() {
            eprintln!(
                "No running waybar-module-pomodoro instance {} found",
                instance
            );
            return Ok(());
        }
        debug!("Targeting instance {}", instance);
    }

    if sockets.is_empty() {
        eprintln!("No running waybar-module-pomodoro module found");
        return Ok(());
    }

    for socket in &sockets {
        debug!("Socket path: {}", socket.display());
    }

    let message = cli.operation.to_message().encode();

    let mut success_count = 0;
    for socket in sockets {
        let socket_str = socket.to_string_lossy();
        debug!("Sending message '{}' to socket '{}'", message, socket_str);
        match send_message_socket(&socket_str, &message) {
            Ok(_) => {
                debug!("Message sent successfully to {}", socket_str);
                success_count += 1;
            }
            Err(e) => {
                warn!("Failed to send message to {}: {}", socket_str, e);
            }
        }
    }

    if success_count == 0 {
        eprintln!("Failed to send message to any running modules");
    }

    Ok(())
}
