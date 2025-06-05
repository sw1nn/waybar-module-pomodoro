use crate::{
    cli::ModuleCli,
    utils::consts::{
        BREAK_ICON, LONG_BREAK_TIME, MINUTE, PAUSE_ICON, PLAY_ICON, SHORT_BREAK_TIME, WORK_ICON,
        WORK_TIME,
    },
};
use std::env;

#[derive(Debug)]
pub struct Config {
    pub work_time: u16,
    pub short_break: u16,
    pub long_break: u16,
    pub no_icons: bool,
    pub no_work_icons: bool,
    pub play_icon: String,
    pub pause_icon: String,
    pub work_icon: String,
    pub break_icon: String,
    pub work_sound: Option<String>,
    pub break_sound: Option<String>,
    pub autow: bool,
    pub autob: bool,
    pub persist: bool,
    pub with_notifications: bool,
    pub binary_name: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            work_time: Default::default(),
            short_break: Default::default(),
            long_break: Default::default(),
            no_icons: Default::default(),
            no_work_icons: Default::default(),
            play_icon: PLAY_ICON.to_string(),
            pause_icon: PAUSE_ICON.to_string(),
            work_icon: WORK_ICON.to_string(),
            break_icon: BREAK_ICON.to_string(),
            work_sound: Default::default(),
            break_sound: Default::default(),
            autow: Default::default(),
            autob: Default::default(),
            persist: Default::default(),
            with_notifications: Default::default(),
            binary_name: Default::default(),
        }
    }
}

impl Config {
    pub fn from_module_cli(cli: &ModuleCli) -> Self {
        let binary_name = env::current_exe()
            .ok()
            .and_then(|path| path.file_name().map(|s| s.to_owned()))
            .and_then(|s| s.to_str().map(|s| s.to_string()))
            .unwrap_or_else(|| "waybar-module-pomodoro".to_string());

        let config = Self {
            work_time: cli.work.map(|w| w * MINUTE).unwrap_or(WORK_TIME),
            short_break: cli
                .shortbreak
                .map(|s| s * MINUTE)
                .unwrap_or(SHORT_BREAK_TIME),
            long_break: cli.longbreak.map(|l| l * MINUTE).unwrap_or(LONG_BREAK_TIME),
            no_icons: cli.no_icons,
            no_work_icons: cli.no_work_icons,
            play_icon: cli.play.clone().unwrap_or_else(|| PLAY_ICON.to_string()),
            pause_icon: cli.pause.clone().unwrap_or_else(|| PAUSE_ICON.to_string()),
            work_icon: cli
                .work_icon
                .clone()
                .unwrap_or_else(|| WORK_ICON.to_string()),
            break_icon: cli
                .break_icon
                .clone()
                .unwrap_or_else(|| BREAK_ICON.to_string()),
            work_sound: cli.work_sound.clone(),
            break_sound: cli.break_sound.clone(),
            autow: cli.autow,
            autob: cli.autob,
            persist: cli.persist,
            with_notifications: cli.with_notifications,
            binary_name,
        };

        tracing::debug!("Created config from CLI: {:#?}", config);
        config
    }

    pub fn get_play_pause_icon(&self, running: bool) -> &str {
        if self.no_icons {
            return "";
        }

        if !running {
            &self.play_icon
        } else {
            &self.pause_icon
        }
    }

    pub fn get_cycle_icon(&self, is_break: bool) -> &str {
        if self.no_work_icons {
            return "";
        }

        if !is_break {
            &self.work_icon
        } else {
            &self.break_icon
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_play_pause_icon_running() {
        let config = Config::default();
        let icon = config.get_play_pause_icon(true);

        assert_eq!(icon, PAUSE_ICON);
    }

    #[test]
    fn test_get_play_pause_icon_not_running() {
        let config = Config::default();
        let icon = config.get_play_pause_icon(false);

        assert_eq!(icon, PLAY_ICON);
    }

    #[test]
    fn test_get_play_pause_icon_no_icons() {
        let config = Config {
            no_icons: true,
            ..Default::default()
        };
        let icon = config.get_play_pause_icon(true);

        assert_eq!(icon, "");
    }

    #[test]
    fn test_config_from_module_cli_defaults() {
        use crate::cli::ModuleCli;
        use clap::Parser;

        let cli = ModuleCli::try_parse_from(vec!["waybar-module-pomodoro"]).unwrap();
        let config = Config::from_module_cli(&cli);

        assert_eq!(config.work_time, WORK_TIME);
        assert_eq!(config.short_break, SHORT_BREAK_TIME);
        assert_eq!(config.long_break, LONG_BREAK_TIME);
        assert!(!config.no_icons);
        assert!(!config.no_work_icons);
        assert_eq!(config.play_icon, PLAY_ICON.to_string());
        assert_eq!(config.pause_icon, PAUSE_ICON.to_string());
        assert_eq!(config.work_icon, WORK_ICON.to_string());
        assert_eq!(config.break_icon, BREAK_ICON.to_string());
        assert!(!config.autow);
        assert!(!config.autob);
        assert!(!config.persist);
    }

    #[test]
    fn test_config_from_module_cli_with_options() {
        use crate::cli::ModuleCli;
        use clap::Parser;

        let cli = ModuleCli::try_parse_from(vec![
            "waybar-module-pomodoro",
            "--work",
            "30",
            "--shortbreak",
            "10",
            "--autow",
            "--persist",
        ])
        .unwrap();
        let config = Config::from_module_cli(&cli);

        assert_eq!(config.work_time, 30 * MINUTE);
        assert_eq!(config.short_break, 10 * MINUTE);
        assert_eq!(config.long_break, LONG_BREAK_TIME);
        assert!(config.autow);
        assert!(!config.autob);
        assert!(config.persist);
    }
}
