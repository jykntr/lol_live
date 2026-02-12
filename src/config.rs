use std::fs;
use std::path::PathBuf;

pub struct LeagueConfig {
    pub hud_scale: f64,
    pub game_width: u32,
    pub game_height: u32,
    pub window_mode: u32,
}

impl Default for LeagueConfig {
    fn default() -> Self {
        Self {
            hud_scale: 1.0,
            game_width: 1920,
            game_height: 1080,
            window_mode: 1,
        }
    }
}

impl std::fmt::Display for LeagueConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "hud_scale={:.2}, game_res={}x{}, window_mode={}",
            self.hud_scale, self.game_width, self.game_height, self.window_mode
        )
    }
}

fn config_path() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME").ok()?;
        Some(
            PathBuf::from(home)
                .join("Library/Application Support/Riot Games/League of Legends/Config/game.cfg"),
        )
    }

    #[cfg(target_os = "windows")]
    {
        Some(PathBuf::from(
            r"C:\Riot Games\League of Legends\Config\game.cfg",
        ))
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        None
    }
}

pub fn load() -> LeagueConfig {
    let mut config = LeagueConfig::default();

    let path = match config_path() {
        Some(p) => p,
        None => return config,
    };

    let contents = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return config,
    };

    let mut current_section = String::new();

    for line in contents.lines() {
        let line = line.trim();

        if line.starts_with('[') && line.ends_with(']') {
            current_section = line[1..line.len() - 1].to_string();
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();

        match current_section.as_str() {
            "HUD" => {
                if key == "GlobalScale" {
                    if let Ok(v) = value.parse::<f64>() {
                        config.hud_scale = v;
                    }
                }
            }
            "General" => match key {
                "Width" => {
                    if let Ok(v) = value.parse::<u32>() {
                        config.game_width = v;
                    }
                }
                "Height" => {
                    if let Ok(v) = value.parse::<u32>() {
                        config.game_height = v;
                    }
                }
                "WindowMode" => {
                    if let Ok(v) = value.parse::<u32>() {
                        config.window_mode = v;
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }

    config
}
