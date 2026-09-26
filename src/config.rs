use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub fn home() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_else(|| "/".into()))
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Live,
    Static,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Config {
    pub mode: Mode,
    pub interval_minutes: u64,
    pub fps: u32,
    pub font_family: String,
    pub font_size: f32,
    pub scenes: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            mode: Mode::Live,
            interval_minutes: 30,
            fps: 30,
            font_family: "JetBrainsMono Nerd Font Mono".into(),
            font_size: 16.0,
            scenes: ["waves", "plasma", "noise", "tunnel", "matrix", "life", "maze", "shaders", "images"]
                .map(String::from)
                .to_vec(),
        }
    }
}

impl Config {
    pub fn path() -> PathBuf {
        home().join(".config/asciiwall/config.toml")
    }

    pub fn load() -> Result<Config> {
        let path = Self::path();
        match std::fs::read_to_string(&path) {
            Ok(s) => toml::from_str(&s).with_context(|| format!("invalid config {}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
            Err(e) => Err(e).with_context(|| format!("cannot read {}", path.display())),
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path();
        std::fs::create_dir_all(path.parent().unwrap())?;
        std::fs::write(&path, toml::to_string_pretty(self)?)
            .with_context(|| format!("cannot write {}", path.display()))
    }

    /// No timer when rotation is disabled; manual scene changes still work.
    pub fn interval(&self) -> Option<std::time::Duration> {
        (self.interval_minutes > 0)
            .then(|| std::time::Duration::from_secs(self.interval_minutes.saturating_mul(60)))
    }
}
