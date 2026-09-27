use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
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

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Tempo {
    Low,
    Medium,
    High,
}

impl Tempo {
    pub fn as_str(self) -> &'static str {
        match self {
            Tempo::Low => "low",
            Tempo::Medium => "medium",
            Tempo::High => "high",
        }
    }

    /// Half, written, or double pace. Natural is whatever the scene was authored at.
    pub fn factor(self) -> f32 {
        match self {
            Tempo::Low => 0.5,
            Tempo::Medium => 1.0,
            Tempo::High => 2.0,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Config {
    /// Off hands the desktop back to the stock Omarchy background. The struct-level
    /// `serde(default)` makes a file written before this key existed read as on.
    pub enabled: bool,
    pub mode: Mode,
    pub interval_minutes: u64,
    pub fps: u32,
    pub font_family: String,
    pub font_size: f32,
    pub scenes: Vec<String>,
    /// Per-scene override. Missing means Natural, so a new config changes nothing.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub tempo: BTreeMap<String, Tempo>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            enabled: true,
            mode: Mode::Live,
            interval_minutes: 30,
            fps: 30,
            font_family: "JetBrainsMono Nerd Font Mono".into(),
            font_size: 16.0,
            scenes: [
                "waves", "plasma", "noise", "tunnel", "matrix", "life", "maze", "plates", "shaders",
            ]
            .map(String::from)
            .to_vec(),
            tempo: BTreeMap::new(),
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
            Ok(s) => {
                toml::from_str(&s).with_context(|| format!("invalid config {}", path.display()))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
            Err(e) => Err(e).with_context(|| format!("cannot read {}", path.display())),
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path();
        std::fs::create_dir_all(path.parent().unwrap())?;
        let body = toml::to_string_pretty(self)?;
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, body).with_context(|| format!("cannot write {}", tmp.display()))?;
        std::fs::rename(&tmp, &path).with_context(|| format!("cannot write {}", path.display()))
    }

    pub fn tempo_of(&self, scene: &str) -> Tempo {
        self.tempo.get(scene).copied().unwrap_or(Tempo::Medium)
    }

    /// Natural removes the override, so the file only records a deliberate change.
    pub fn set_tempo(&mut self, scene: &str, tempo: Tempo) {
        if tempo == Tempo::Medium {
            self.tempo.remove(scene);
        } else {
            self.tempo.insert(scene.to_string(), tempo);
        }
    }

    /// No timer when rotation is disabled; manual scene changes still work.
    pub fn interval(&self) -> Option<std::time::Duration> {
        (self.interval_minutes > 0)
            .then(|| std::time::Duration::from_secs(self.interval_minutes.saturating_mul(60)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_without_enabled_key_stays_on() {
        let cfg: Config = toml::from_str("mode = \"static\"\ninterval_minutes = 15\n").unwrap();
        assert!(cfg.enabled);
        let off: Config = toml::from_str("enabled = false\n").unwrap();
        assert!(!off.enabled);
    }
}
