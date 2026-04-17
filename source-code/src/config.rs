use anyhow::{Context, Result};
use dirs::home_dir;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HnmConfig {
    pub nix_channel: String,
    pub profile_dir: String,
    pub auto_gc: bool,
    pub max_generations: u32,
}

impl Default for HnmConfig {
    fn default() -> Self {
        Self {
            nix_channel: "https://nixos.org/channels/nixpkgs-unstable".into(),
            profile_dir: profile_dir().to_string_lossy().into(),
            auto_gc: false,
            max_generations: 10,
        }
    }
}

pub fn home() -> PathBuf {
    home_dir().unwrap_or_else(|| PathBuf::from("/root"))
}

pub fn config_dir() -> PathBuf {
    home().join(".config").join("hnm")
}

pub fn profile_dir() -> PathBuf {
    home().join(".hnm").join("profile")
}

pub fn data_dir() -> PathBuf {
    home().join(".local").join("share").join("hnm")
}

pub fn load() -> Result<HnmConfig> {
    let path = config_dir().join("config.toml");
    if !path.exists() {
        let cfg = HnmConfig::default();
        save(&cfg)?;
        return Ok(cfg);
    }
    let raw = fs::read_to_string(&path)
    .with_context(|| format!("cannot read config: {:?}", path))?;
    toml::from_str(&raw).with_context(|| "invalid config.toml")
}

pub fn save(cfg: &HnmConfig) -> Result<()> {
    fs::create_dir_all(config_dir())?;
    let raw = toml::to_string_pretty(cfg)?;
    fs::write(config_dir().join("config.toml"), raw)?;
    Ok(())
}
