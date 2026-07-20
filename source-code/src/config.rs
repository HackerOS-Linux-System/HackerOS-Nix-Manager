use anyhow::{anyhow, Context, Result};
use dirs::home_dir;
use hk_parser::{load_hk_file, write_hk_file, HkConfig, HkValue};
use indexmap::IndexMap;
use std::path::PathBuf;

#[derive(Debug, Clone)]
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

/// Path to the config file. Historically `config.toml`; as of v0.2 this
/// is `config.hk` (see docs/hnm.html#config). If an old `config.toml`
/// is found and `config.hk` is not, we migrate it automatically so
/// existing installs don't lose their settings on upgrade.
pub fn config_path() -> PathBuf {
    config_dir().join("config.hk")
}

fn legacy_config_path() -> PathBuf {
    config_dir().join("config.toml")
}

// ─── very small legacy TOML reader ────────────────────────────────────────
// Just enough to migrate the handful of flat `key = value` lines HNM's old
// config.toml ever wrote — not a general TOML parser. If parsing fails we
// simply fall back to defaults instead of hard-erroring the migration.
fn migrate_legacy_toml(raw: &str) -> HnmConfig {
    let mut cfg = HnmConfig::default();
    for line in raw.lines() {
        let line = line.trim();
        let Some((key, val)) = line.split_once('=') else { continue };
        let key = key.trim();
        let val = val.trim().trim_matches('"');
        match key {
            "nix_channel" => cfg.nix_channel = val.to_string(),
            "profile_dir" => cfg.profile_dir = val.to_string(),
            "auto_gc" => cfg.auto_gc = val.eq_ignore_ascii_case("true"),
            "max_generations" => {
                if let Ok(n) = val.parse::<u32>() {
                    cfg.max_generations = n;
                }
            }
            _ => {}
        }
    }
    cfg
}

// ─── HkValue <-> HnmConfig conversion ─────────────────────────────────────

fn to_hk(cfg: &HnmConfig) -> HkConfig {
    let mut section: IndexMap<String, HkValue> = IndexMap::new();
    section.insert("nix_channel".into(), HkValue::String(cfg.nix_channel.clone()));
    section.insert("profile_dir".into(), HkValue::String(cfg.profile_dir.clone()));
    section.insert("auto_gc".into(), HkValue::Bool(cfg.auto_gc));
    section.insert(
        "max_generations".into(),
        HkValue::Number(cfg.max_generations as f64),
    );

    let mut root: HkConfig = IndexMap::new();
    root.insert("hnm".into(), HkValue::Map(section));
    root
}

fn from_hk(hk: &HkConfig) -> Result<HnmConfig> {
    let default = HnmConfig::default();

    let section = match hk.get("hnm") {
        Some(v) => v
            .as_map()
            .map_err(|e| anyhow!("[hnm] section in config.hk is malformed: {}", e))?,
        None => return Err(anyhow!("config.hk is missing the [hnm] section")),
    };

    let nix_channel = section
        .get("nix_channel")
        .and_then(|v| v.as_string().ok())
        .unwrap_or(default.nix_channel);

    let profile_dir = section
        .get("profile_dir")
        .and_then(|v| v.as_string().ok())
        .unwrap_or(default.profile_dir);

    let auto_gc = section
        .get("auto_gc")
        .and_then(|v| v.as_bool().ok())
        .unwrap_or(default.auto_gc);

    let max_generations = section
        .get("max_generations")
        .and_then(|v| v.as_number().ok())
        .map(|n| n.max(0.0) as u32)
        .unwrap_or(default.max_generations);

    Ok(HnmConfig {
        nix_channel,
        profile_dir,
        auto_gc,
        max_generations,
    })
}

// ─── public API ────────────────────────────────────────────────────────────

pub fn load() -> Result<HnmConfig> {
    let path = config_path();

    if !path.exists() {
        // First run — or migrating from the old config.toml.
        let cfg = if legacy_config_path().exists() {
            let raw = std::fs::read_to_string(legacy_config_path())
                .with_context(|| "cannot read legacy config.toml")?;
            let cfg = migrate_legacy_toml(&raw);
            crate::output::info("migrated ~/.config/hnm/config.toml → config.hk");
            cfg
        } else {
            HnmConfig::default()
        };
        save(&cfg)?;
        return Ok(cfg);
    }

    let hk = load_hk_file(&path)
        .map_err(|e| anyhow!("cannot parse {}: {}", path.display(), e))?;

    from_hk(&hk)
}

pub fn save(cfg: &HnmConfig) -> Result<()> {
    std::fs::create_dir_all(config_dir())
        .with_context(|| format!("cannot create {}", config_dir().display()))?;

    let hk = to_hk(cfg);
    write_hk_file(&config_path(), &hk)
        .with_context(|| format!("cannot write {}", config_path().display()))?;
    Ok(())
}
