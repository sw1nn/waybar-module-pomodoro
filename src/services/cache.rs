use std::{env, error::Error, fs::File, io::Write, path::PathBuf};

use crate::models::config::Config;

use super::timer::Timer;

const MODULE: &str = env!("CARGO_PKG_NAME");
const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn store(state: &Timer) -> Result<(), Box<dyn Error>> {
    let mut filepath = cache_dir()?;
    let output_name = format!("{MODULE}-{VERSION}");
    filepath.push(output_name);

    store_to_path(state, &filepath)
}

fn store_to_path(state: &Timer, filepath: &std::path::Path) -> Result<(), Box<dyn Error>> {
    let data = serde_json::to_string(&state).expect("Not a serializable type");
    Ok(File::create(filepath)?.write_all(data.as_bytes())?)
}

fn restore_from_path(
    state: &mut Timer,
    config: &Config,
    filepath: &std::path::Path,
) -> Result<(), Box<dyn Error>> {
    let mut file = File::open(filepath)?;
    let mut content = String::new();
    std::io::Read::read_to_string(&mut file, &mut content)?;

    let restored: Timer = serde_json::from_str(&content)?;

    if match_timers(config, &restored.times) {
        state.current_index = restored.current_index;
        state.elapsed_millis = restored.elapsed_millis;
        state.elapsed_time = restored.elapsed_time;
        state.times = restored.times;
        state.iterations = restored.iterations;
        state.session_completed = restored.session_completed;
        state.running = restored.running;
    }

    Ok(())
}

pub fn restore(state: &mut Timer, config: &Config) -> Result<(), Box<dyn Error>> {
    let mut filepath = cache_dir()?;
    let output_name = format!("{MODULE}-{VERSION}");
    filepath.push(output_name);

    restore_from_path(state, config, &filepath)
}

fn match_timers(config: &Config, times: &[u16; 3]) -> bool {
    let work_time: u16 = times[0];
    let short_break: u16 = times[1];
    let long_break: u16 = times[2];

    if config.work_time != work_time
        || config.short_break != short_break
        || config.long_break != long_break
    {
        return false;
    }

    true
}

fn cache_dir() -> Result<PathBuf, Box<dyn Error>> {
    let mut dir = if let Some(dir) = dirs::cache_dir() {
        dir
    } else {
        return Err("unable to get cache dir".into());
    };

    dir.push(MODULE);
    if let Err(e) = std::fs::create_dir_all(&dir) {
        println!("create_dir: path == {:?}, err == {e}", dir);
    }
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    // Removed unused test functions

    fn create_timer(
        work_time: Option<u16>,
        short_break: Option<u16>,
        long_break: Option<u16>,
    ) -> Timer {
        Timer {
            current_index: 1,
            elapsed_millis: 950,
            elapsed_time: 300,
            times: [
                work_time.unwrap_or(25),
                short_break.unwrap_or(5),
                long_break.unwrap_or(15),
            ],
            iterations: 2,
            session_completed: 8,
            running: false, // Default to false, we'll set it explicitly in tests when needed
            socket_nr: 0,
            current_override: None,
        }
    }

    #[test]
    fn test_store_and_restore() -> Result<(), Box<dyn Error>> {
        // Create a temporary file for testing
        let temp_file = NamedTempFile::new()?;
        let temp_path = temp_file.path();

        // Create a timer with running=true
        let mut timer = create_timer(None, None, None);
        timer.running = true; // Set the running state to true for testing

        // Store to temp file
        store_to_path(&timer, temp_path)?;

        // Create a timer with different values to restore into
        let mut restored_timer = create_timer(Some(30), Some(10), Some(20));

        // Config that matches the stored timer
        let config = Config {
            work_time: 25,
            short_break: 5,
            long_break: 15,
            ..Default::default()
        };

        // Restore from temp file
        restore_from_path(&mut restored_timer, &config, temp_path)?;

        // Verify all fields were correctly restored
        assert_eq!(restored_timer.current_index, timer.current_index);
        assert_eq!(restored_timer.elapsed_millis, timer.elapsed_millis);
        assert_eq!(restored_timer.elapsed_time, timer.elapsed_time);
        assert_eq!(restored_timer.times, timer.times);
        assert_eq!(restored_timer.iterations, timer.iterations);
        assert_eq!(restored_timer.session_completed, timer.session_completed);
        assert_eq!(restored_timer.running, timer.running);

        Ok(())
    }

    #[test]
    fn test_store_and_restore_mismatched_config() -> Result<(), Box<dyn Error>> {
        // Create a temporary file for testing
        let temp_file = NamedTempFile::new()?;
        let temp_path = temp_file.path();

        // Create and store a timer
        let timer = create_timer(Some(25), Some(5), Some(15));
        store_to_path(&timer, temp_path)?;

        // Create a timer with different times to restore into
        let mut restored_timer = create_timer(Some(30), Some(10), Some(20));
        let original_times = restored_timer.times;

        // Config with mismatched times
        let config = Config {
            work_time: 30,
            short_break: 10,
            long_break: 20,
            ..Default::default()
        };

        // Try to restore from temp file
        restore_from_path(&mut restored_timer, &config, temp_path)?;

        // Times should not match, so timer should remain unchanged
        assert_eq!(restored_timer.times, original_times);

        Ok(())
    }

    #[test]
    fn test_persist_running_state() -> Result<(), Box<dyn Error>> {
        // Create a temporary file for testing
        let temp_file = NamedTempFile::new()?;
        let temp_path = temp_file.path();

        // Create a timer with running=true and store it
        let mut timer = create_timer(None, None, None);
        timer.running = true;
        store_to_path(&timer, temp_path)?;

        // Create a new timer with running=false to restore into
        let mut restored_timer = create_timer(None, None, None);
        assert!(
            !restored_timer.running,
            "New timer should not be running by default"
        );

        // Config that matches the stored timer
        let config = Config {
            work_time: 25,
            short_break: 5,
            long_break: 15,
            ..Default::default()
        };

        // Restore from temp file
        restore_from_path(&mut restored_timer, &config, temp_path)?;

        // Verify running state was restored
        assert!(
            restored_timer.running,
            "Running state should be restored to true"
        );

        Ok(())
    }

    #[test]
    fn test_cache_dir_creation() -> Result<(), Box<dyn Error>> {
        // We don't need to set env vars as we're not testing the cache path directly
        let result = cache_dir()?;

        // Just verify the cache directory exists and is a directory
        assert!(result.exists());
        assert!(result.is_dir());

        Ok(())
    }

    #[test]
    fn test_match_timers_match() {
        let config = Config {
            work_time: 25,
            short_break: 5,
            long_break: 15,
            ..Default::default()
        };

        let times = [25, 5, 15];

        assert!(match_timers(&config, &times));
    }

    #[test]
    fn test_match_timers_mismatch() {
        let config = Config {
            work_time: 30,
            short_break: 10,
            long_break: 20,
            ..Default::default()
        };

        let times = [25, 5, 15];

        assert!(!match_timers(&config, &times));
    }
}
