use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::PathBuf};
use crate::config::data_dir;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InstalledPkg {
    pub name: String,
    pub version: String,
    pub attr_path: String,
    pub installed_at: DateTime<Utc>,
    pub pinned: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct State {
    pub installed: HashMap<String, InstalledPkg>,
    pub generation: u32,
    pub last_update: Option<DateTime<Utc>>,
}

fn path() -> PathBuf { data_dir().join("state.json") }

pub fn load() -> Result<State> {
    let p = path();
    if !p.exists() { return Ok(State::default()); }
    let raw = fs::read_to_string(&p).with_context(|| "cannot read HNM state")?;
    serde_json::from_str(&raw).with_context(|| "invalid HNM state JSON")
}

pub fn save(st: &State) -> Result<()> {
    fs::create_dir_all(data_dir())?;
    fs::write(path(), serde_json::to_string_pretty(st)?)?;
    Ok(())
}

pub fn is_installed(name: &str) -> bool {
    load().map(|s| s.installed.contains_key(name)).unwrap_or(false)
}

pub fn get(name: &str) -> Option<InstalledPkg> {
    load().ok()?.installed.remove(name)
}

pub fn add(pkg: InstalledPkg) -> Result<()> {
    let mut st = load()?;
    st.installed.insert(pkg.name.clone(), pkg);
    save(&st)
}

pub fn remove(name: &str) -> Result<bool> {
    let mut st = load()?;
    let removed = st.installed.remove(name).is_some();
    if removed { save(&st)?; }
    Ok(removed)
}

pub fn list() -> Result<Vec<InstalledPkg>> {
    let st = load()?;
    let mut v: Vec<InstalledPkg> = st.installed.into_values().collect();
    v.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(v)
}

pub fn pin(name: &str, version: &str) -> Result<()> {
    let mut st = load()?;
    if let Some(pkg) = st.installed.get_mut(name) {
        pkg.pinned = Some(version.to_string());
        save(&st)?;
    }
    Ok(())
}

pub fn unpin(name: &str) -> Result<()> {
    let mut st = load()?;
    if let Some(pkg) = st.installed.get_mut(name) {
        pkg.pinned = None;
        save(&st)?;
    }
    Ok(())
}
