use serde::{Deserialize, Serialize};

use crate::{
    models::config::Config,
    utils::consts::{MAX_ITERATIONS, SLEEP_TIME},
};

use super::module::send_notification;

use tracing::debug;

// CSS class constants
const CLASS_EMPTY: &str = "";
const CLASS_PAUSE: &str = "pause";
const CLASS_WORK: &str = "work";
const CLASS_BREAK: &str = "break";

#[derive(Debug)]
pub enum CycleType {
    Work,
    ShortBreak,
    LongBreak,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Timer {
    pub current_index: usize,
    pub elapsed_millis: u16,
    pub elapsed_time: u16,
    pub times: [u16; 3],
    pub iterations: u8,
    pub session_completed: u8,
    pub running: bool,
    pub socket_nr: i32,
    #[serde(skip)]
    pub current_override: Option<u16>,
}

impl Timer {
    pub fn new(work_time: u16, short_break: u16, long_break: u16, socker_nr: i32) -> Timer {
        Timer {
            current_index: 0,
            elapsed_millis: 0,
            elapsed_time: 0,
            times: [work_time, short_break, long_break],
            iterations: 0,
            session_completed: 0,
            running: false,
            socket_nr: socker_nr,
            current_override: None,
        }
    }

    pub fn reset(&mut self) {
        self.current_index = 0;
        self.elapsed_time = 0;
        self.elapsed_millis = 0;
        self.iterations = 0;
        self.running = false;
        self.current_override = None;
    }

    pub fn is_break(&self) -> bool {
        self.current_index != 0
    }

    pub fn set_time(&mut self, cycle: CycleType, input: u16) {
        self.reset();

        match cycle {
            CycleType::Work => self.times[0] = input * 60,
            CycleType::ShortBreak => self.times[1] = input * 60,
            CycleType::LongBreak => self.times[2] = input * 60,
        }
        println!("{:?}", self.times);
    }

    pub fn add_delta_time(&mut self, cycle: CycleType, delta: i16) {
        let index = match cycle {
            CycleType::Work => 0,
            CycleType::ShortBreak => 1,
            CycleType::LongBreak => 2,
        };

        let delta_seconds = delta * 60;
        let current_time = self.times[index] as i32;
        let new_time = (current_time + delta_seconds as i32).max(0) as u16;

        // If we're modifying the current active cycle and the time goes to zero
        if new_time == 0 && self.current_index == index {
            // Gracefully transition to next state by setting elapsed time to max
            self.elapsed_time = self.times[index];
            self.elapsed_millis = 0;
        } else {
            self.times[index] = new_time;
        }

        println!("{:?}", self.times);
    }

    pub fn set_current_duration(&mut self, minutes: u16) {
        let new_duration = minutes * 60;
        self.current_override = Some(new_duration);
        // Reset elapsed time if we set it to less than current elapsed
        if self.elapsed_time > new_duration {
            self.elapsed_time = new_duration;
            self.elapsed_millis = 0;
        }
        debug!("Current cycle overridden to {} seconds", new_duration);
    }

    pub fn add_current_delta_time(&mut self, delta: i16) {
        let delta_seconds = delta * 60;
        let current_time = self.get_current_time() as i32;
        let new_time = (current_time + delta_seconds as i32).max(0) as u16;

        // If the time goes to zero, gracefully transition
        if new_time == 0 {
            self.elapsed_time = self.get_current_time();
            self.elapsed_millis = 0;
            self.current_override = Some(0);
        } else {
            self.current_override = Some(new_time);
            // Adjust elapsed time if necessary
            if self.elapsed_time > new_time {
                self.elapsed_time = new_time;
                self.elapsed_millis = 0;
            }
        }

        debug!(
            "Current cycle adjusted by {} to {} seconds",
            delta_seconds, new_time
        );
        println!("{:?}", self.times);
    }

    pub fn get_class(&self) -> &'static str {
        // timer hasn't been started yet
        if self.elapsed_millis == 0
            && self.elapsed_time == 0
            && self.iterations == 0
            && self.session_completed == 0
        {
            CLASS_EMPTY
        }
        // timer has been paused
        else if !self.running {
            CLASS_PAUSE
        }
        // currently doing some work
        else if !self.is_break() {
            CLASS_WORK
        }
        // currently a break
        else if self.is_break() {
            CLASS_BREAK
        } else {
            panic!("invalid condition occurred while setting class!");
        }
    }

    pub fn update_state(&mut self, config: &Config) {
        if (self.get_current_time() - self.elapsed_time) == 0 {
            // Clear any override when transitioning to a new cycle
            self.current_override = None;

            // if we're on the third iteration and first work, then we want a long break
            if self.current_index == 0 && self.iterations == MAX_ITERATIONS - 1 {
                self.current_index = self.times.len() - 1;
                self.iterations = MAX_ITERATIONS;
            }
            // if we've had our long break, reset everything and start over
            else if self.current_index == self.times.len() - 1
                && self.iterations == MAX_ITERATIONS
            {
                self.current_index = 0;
                self.iterations = 0;
                // since we've gone through a long break, we've also completed a single pomodoro!
                self.session_completed += 1;
            }
            // otherwise, run as normal
            else {
                self.current_index = (self.current_index + 1) % 2;
                if self.current_index == 0 {
                    self.iterations += 1;
                }
            }

            self.elapsed_time = 0;

            // if the user has passed either auto flag, we want to keep ticking the timer
            // NOTE: the is_break() seems to be flipped..?
            self.running = (config.autob && self.is_break()) || (config.autow && !self.is_break());

            // only send a notification for the first instance of the module
            if self.socket_nr == 0 {
                send_notification(
                    match self.current_index {
                        0 => CycleType::Work,
                        1 => CycleType::ShortBreak,
                        2 => CycleType::LongBreak,
                        _ => panic!("Invalid cycle type"),
                    },
                    config,
                );
            } else {
                debug!(socket_nr = self.socket_nr, "didn't send a notification");
            }
        }
    }

    pub fn get_current_time(&self) -> u16 {
        self.current_override
            .unwrap_or(self.times[self.current_index])
    }

    pub fn increment_time(&mut self) {
        self.elapsed_millis += SLEEP_TIME;
        if self.elapsed_millis >= 1000 {
            self.elapsed_millis = 0;
            self.elapsed_time += 1;
        }
    }

    pub fn next_state(&mut self, config: &Config) {
        // Skip to end of current timer
        self.elapsed_time = self.get_current_time();
        self.elapsed_millis = 0;

        // Trigger state transition
        self.update_state(config);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::consts::{LONG_BREAK_TIME, SHORT_BREAK_TIME, SLEEP_DURATION, WORK_TIME};

    fn create_timer() -> Timer {
        Timer::new(WORK_TIME, SHORT_BREAK_TIME, LONG_BREAK_TIME, 0)
    }

    #[test]
    fn test_new_timer() {
        let timer = create_timer();

        assert_eq!(timer.current_index, 0);
        assert_eq!(timer.elapsed_millis, 0);
        assert_eq!(timer.elapsed_time, 0);
        assert_eq!(timer.times, [WORK_TIME, SHORT_BREAK_TIME, LONG_BREAK_TIME]);
        assert_eq!(timer.iterations, 0);
        assert_eq!(timer.session_completed, 0);
        assert!(!timer.running);
    }

    #[test]
    fn test_reset_timer() {
        let mut timer = create_timer();
        timer.current_index = 2;
        timer.elapsed_millis = 999;
        timer.elapsed_time = WORK_TIME - 1;
        timer.iterations = 4;
        timer.session_completed = 3;
        timer.running = true;

        timer.reset();

        assert_eq!(timer.current_index, 0);
        assert_eq!(timer.elapsed_millis, 0);
        assert_eq!(timer.elapsed_time, 0);
        assert_eq!(timer.iterations, 0);
        assert!(!timer.running);
    }

    #[test]
    fn test_is_break() {
        let mut timer = create_timer();

        assert!(!timer.is_break());

        timer.current_index = 1;
        assert!(timer.is_break());
    }

    #[test]
    fn test_set_time() {
        let mut timer = create_timer();

        timer.set_time(CycleType::Work, 30);
        assert_eq!(timer.times[0], 30 * 60);

        timer.set_time(CycleType::ShortBreak, 10);
        assert_eq!(timer.times[1], 10 * 60);

        timer.set_time(CycleType::LongBreak, 20);
        assert_eq!(timer.times[2], 20 * 60);
    }

    #[test]
    fn test_get_class() {
        let mut timer = create_timer();

        assert_eq!(timer.get_class(), CLASS_EMPTY);

        timer.running = true;
        timer.elapsed_millis = 1;
        assert_eq!(timer.get_class(), CLASS_WORK);

        timer.current_index = 1;
        assert_eq!(timer.get_class(), CLASS_BREAK);

        timer.running = false;
        assert_eq!(timer.get_class(), CLASS_PAUSE);
    }

    #[test]
    fn test_update_state() {
        let mut timer = create_timer();
        let config = Config::default();

        // set to low times so the test passes faster
        let time = 1;
        timer.times[0] = time;
        timer.times[1] = time;
        timer.times[2] = time;

        // Initial state
        assert_eq!(timer.current_index, 0);
        assert_eq!(timer.iterations, 0);

        // Update state after work time is completed
        for _ in 0..time * 1000 / SLEEP_TIME {
            timer.increment_time();
            std::thread::sleep(SLEEP_DURATION);
        }
        timer.update_state(&config);
        assert_eq!(timer.current_index, 1); // Move to short break

        // Update state after short break is completed
        for _ in 0..time * 1000 / SLEEP_TIME {
            timer.increment_time();
            std::thread::sleep(SLEEP_DURATION);
        }
        timer.update_state(&config);

        // we need to trigger a long break
        timer.iterations = MAX_ITERATIONS - 1;

        // Update state after short break is completed
        for _ in 0..time * 1000 / SLEEP_TIME {
            timer.increment_time();
            std::thread::sleep(SLEEP_DURATION);
        }

        timer.update_state(&config);
        assert_eq!(timer.current_index, 2); // Move to long break
    }

    #[test]
    fn test_increment_elapsed_time() {
        let mut timer = create_timer();

        assert_eq!(timer.elapsed_millis, 0);
        assert_eq!(timer.elapsed_time, 0);

        timer.increment_time();
        assert_eq!(timer.elapsed_millis, SLEEP_TIME); // Assuming SLEEP_INTERVAL is defined
        assert_eq!(timer.elapsed_time, 0);

        for _ in 1..SLEEP_TIME {
            timer.increment_time();
        }
        assert_eq!(timer.elapsed_millis, 0);
        assert_eq!(timer.elapsed_time, 10);
    }

    #[test]
    fn test_next_state() {
        let mut timer = create_timer();
        let config = Config::default();

        // Test transitioning from work to short break
        assert_eq!(timer.current_index, 0); // Work
        timer.next_state(&config);
        assert_eq!(timer.current_index, 1); // Short break
        assert_eq!(timer.elapsed_time, 0);

        // Test transitioning from short break to work
        timer.next_state(&config);
        assert_eq!(timer.current_index, 0); // Back to work
        assert_eq!(timer.iterations, 1);

        // Set up for long break transition
        timer.iterations = MAX_ITERATIONS - 1;
        timer.next_state(&config);
        assert_eq!(timer.current_index, 2); // Long break

        // Test transitioning from long break back to work
        timer.next_state(&config);
        assert_eq!(timer.current_index, 0); // Back to work
        assert_eq!(timer.iterations, 0);
        assert_eq!(timer.session_completed, 1); // One session completed
    }
}
