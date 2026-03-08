use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Config {
    pub audio: AudioConfig,
    pub display: DisplayConfig,
    pub keybindings: KeybindingsConfig,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct AudioConfig {
    pub fft_size: usize,
    pub smoothing: f64,
    pub buffer_size: usize,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct DisplayConfig {
    pub fps: u32,
    pub color_mode: String,
    pub show_status_bar: bool,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct KeybindingsConfig {
    pub next_mode: String,
    pub prev_mode: String,
    pub quit: String,
    pub toggle_status: String,
    pub increase_sensitivity: String,
    pub decrease_sensitivity: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            audio: AudioConfig::default(),
            display: DisplayConfig::default(),
            keybindings: KeybindingsConfig::default(),
        }
    }
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            fft_size: 2048,
            smoothing: 0.7,
            buffer_size: 4096,
        }
    }
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            fps: 30,
            color_mode: "truecolor".to_string(),
            show_status_bar: true,
        }
    }
}

impl Default for KeybindingsConfig {
    fn default() -> Self {
        Self {
            next_mode: "Tab".to_string(),
            prev_mode: "Shift+Tab".to_string(),
            quit: "q".to_string(),
            toggle_status: "s".to_string(),
            increase_sensitivity: "+".to_string(),
            decrease_sensitivity: "-".to_string(),
        }
    }
}

impl Config {
    pub fn default_path() -> PathBuf {
        let config_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."));
        config_dir.join("terminal-vibes").join("config.toml")
    }

    pub fn load(path: Option<&PathBuf>) -> anyhow::Result<Self> {
        let config_path = path
            .cloned()
            .unwrap_or_else(Self::default_path);

        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            let config: Config = toml::from_str(&content)?;
            Ok(config)
        } else {
            log::info!("No config file found at {:?}, using defaults", config_path);
            Ok(Config::default())
        }
    }
}
