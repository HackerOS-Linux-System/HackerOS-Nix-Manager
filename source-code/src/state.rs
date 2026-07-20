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

// ─── generation tracking ───────────────────────────────────────────────────
//
// `State.generation` used to be declared and never written anywhere — see
// docs/hnm.html#changelog (v0.2). It's now kept in sync with the real
// nix-env profile generation after every operation that creates one
// (install / remove / update / rollback).

pub fn set_generation(gen: u32) -> Result<()> {
    let mut st = load()?;
    st.generation = gen;
    save(&st)
}

// ─── syncing with the real Nix profile ─────────────────────────────────────
//
// HNM tracks installed packages in its own state.json, separate from
// `nix-env`'s profile. That can drift: a package installed manually with
// `nix-env` won't show up in `hnm list`, and — more importantly — a
// `hnm rollback` switches the underlying profile generation without
// touching state.json, so the tracked package list can go stale. See
// docs/hnm.html#changelog (v0.2).
//
// `sync_from_profile` reconciles state.installed against the packages
// nix-env actually reports as installed right now:
//   - packages present in the profile but missing from state are added
//     (best-effort metadata: install time = now, no pin, no description)
//   - packages tracked in state but no longer present in the profile are
//     dropped
//
// Existing tracked packages that are still installed keep their original
// metadata (install timestamp, pin, description) untouched.
pub fn sync_from_profile(profile_pkgs: &[crate::nix::Pkg]) -> Result<(usize, usize)> {
    let mut st = load()?;

    let profile_names: std::collections::HashSet<&str> =
        profile_pkgs.iter().map(|p| p.name.as_str()).collect();

    // Drop anything HNM thinks is installed but the profile disagrees with.
    let before = st.installed.len();
    st.installed.retain(|name, _| profile_names.contains(name.as_str()));
    let removed = before - st.installed.len();

    // Add anything the profile has that HNM doesn't know about yet.
    let mut added = 0usize;
    for pkg in profile_pkgs {
        if !st.installed.contains_key(&pkg.name) {
            st.installed.insert(
                pkg.name.clone(),
                InstalledPkg {
                    name: pkg.name.clone(),
                    version: pkg.version.clone(),
                    attr_path: pkg.attr_path.clone(),
                    installed_at: Utc::now(),
                    pinned: None,
                    description: if pkg.description.is_empty() {
                        None
                    } else {
                        Some(pkg.description.clone())
                    },
                },
            );
            added += 1;
        }
    }

    if added > 0 || removed > 0 {
        save(&st)?;
    }

    Ok((added, removed))
}
