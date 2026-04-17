use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::process::{Command, Stdio};

use crate::{config, output, progress};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Pkg {
    pub name: String,
    pub version: String,
    pub attr_path: String,
    pub description: String,
    pub homepage: Option<String>,
    pub license: Option<String>,
}

// ─── presence checks ─────────────────────────────────────────────────────────

pub fn nix_ok() -> bool {
    Command::new("nix").arg("--version")
    .stdout(Stdio::null()).stderr(Stdio::null())
    .status().map(|s| s.success()).unwrap_or(false)
}

pub fn nix_env_ok() -> bool {
    Command::new("nix-env").arg("--version")
    .stdout(Stdio::null()).stderr(Stdio::null())
    .status().map(|s| s.success()).unwrap_or(false)
}

/// Ensure Nix is present; auto-install if missing.
pub fn ensure_nix() -> Result<()> {
    if nix_ok() { return Ok(()); }
    output::warn("Nix is not installed — bootstrapping automatically...");
    crate::commands::unpack::run()
}

// ─── search ──────────────────────────────────────────────────────────────────

pub fn search(query: &str, task: &progress::TaskProgress) -> Result<Vec<Pkg>> {
    task.log(&format!("running: nix search nixpkgs {}", query));
    task.set_msg("querying nixpkgs...");

    let out = Command::new("nix")
    .args(["search", "--json", "nixpkgs", query])
    .output()
    .with_context(|| "failed to run `nix search`")?;

    task.inc(40);

    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        task.err_line(&err);
        return Err(anyhow!("nix search failed"));
    }

    task.set_msg("parsing results...");
    let raw = String::from_utf8_lossy(&out.stdout);
    let pkgs = parse_search_json(&raw)?;
    task.inc(60);
    Ok(pkgs)
}

fn parse_search_json(raw: &str) -> Result<Vec<Pkg>> {
    let v: serde_json::Value = serde_json::from_str(raw)
    .with_context(|| "cannot parse nix search output")?;
    let obj = v.as_object().ok_or_else(|| anyhow!("unexpected format"))?;
    let mut out = Vec::new();
    for (attr, info) in obj {
        out.push(Pkg {
            name:        info["pname"].as_str().or_else(|| info["name"].as_str())
            .unwrap_or(attr).to_string(),
                 version:     info["version"].as_str().unwrap_or("?").to_string(),
                 attr_path:   attr.clone(),
                 description: info["description"].as_str().unwrap_or("").to_string(),
                 homepage:    info["homepage"].as_str().map(String::from),
                 license:     info["license"]["spdxId"].as_str()
                 .or_else(|| info["license"].as_str()).map(String::from),
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

// ─── install ─────────────────────────────────────────────────────────────────

pub fn install(attr_path: &str, task: &progress::TaskProgress) -> Result<()> {
    let profile = config::profile_dir();
    task.log(&format!("nix-env --profile {} --install --attr {} --file <nixpkgs>",
                      profile.display(), attr_path));

    let success = progress::run_with_log(task, "nix-env", &[
        "--profile", profile.to_str().unwrap(),
                                         "--install", "--attr", attr_path,
                                         "--file", "<nixpkgs>",
    ])?;

    if !success {
        return Err(anyhow!("nix-env install failed for '{}'", attr_path));
    }
    Ok(())
}

// ─── remove ──────────────────────────────────────────────────────────────────

pub fn remove(pkg_name: &str, task: &progress::TaskProgress) -> Result<()> {
    let profile = config::profile_dir();
    task.log(&format!("nix-env --profile {} --uninstall {}", profile.display(), pkg_name));

    let success = progress::run_with_log(task, "nix-env", &[
        "--profile", profile.to_str().unwrap(),
                                         "--uninstall", pkg_name,
    ])?;

    if !success {
        return Err(anyhow!("nix-env uninstall failed for '{}'", pkg_name));
    }
    Ok(())
}

// ─── update ──────────────────────────────────────────────────────────────────

pub fn update_channel(task: &progress::TaskProgress) -> Result<()> {
    let cfg = config::load()?;
    task.log(&format!("nix-channel --add {} nixpkgs", cfg.nix_channel));
    let _ = Command::new("nix-channel")
    .args(["--add", &cfg.nix_channel, "nixpkgs"])
    .status();

    task.set_msg("nix-channel --update");
    let success = progress::run_with_log(task, "nix-channel", &["--update"])?;
    if !success {
        return Err(anyhow!("nix-channel --update failed"));
    }
    Ok(())
}

pub fn upgrade_profile(task: &progress::TaskProgress) -> Result<()> {
    let profile = config::profile_dir();
    task.log(&format!("nix-env --profile {} --upgrade", profile.display()));
    task.set_msg("upgrading packages...");

    let success = progress::run_with_log(task, "nix-env", &[
        "--profile", profile.to_str().unwrap(),
                                         "--upgrade",
    ])?;
    if !success {
        return Err(anyhow!("nix-env upgrade failed"));
    }
    Ok(())
}

// ─── info ────────────────────────────────────────────────────────────────────

pub fn info(name: &str) -> Result<Pkg> {
    let out = Command::new("nix")
    .args(["eval", "--json", &format!("nixpkgs#{name}.meta")])
    .output()
    .with_context(|| format!("cannot query info for '{name}'"))?;

    if !out.status.success() {
        return Err(anyhow!("package '{}' not found in nixpkgs", name));
    }

    let meta: serde_json::Value = serde_json::from_slice(&out.stdout)
    .with_context(|| "cannot parse package meta")?;

    let ver = Command::new("nix")
    .args(["eval", "--raw", &format!("nixpkgs#{name}.version")])
    .output().ok()
    .and_then(|o| String::from_utf8(o.stdout).ok())
    .map(|s| s.trim().to_string())
    .unwrap_or_else(|| "?".into());

    Ok(Pkg {
        name: name.into(),
       version: ver,
       attr_path: format!("nixpkgs#{name}"),
       description: meta["description"].as_str().unwrap_or("").into(),
       homepage: meta["homepage"].as_str().map(String::from),
       license: meta["license"]["spdxId"].as_str()
       .or_else(|| meta["license"].as_str()).map(String::from),
    })
}

// ─── list installed ──────────────────────────────────────────────────────────

pub fn list_profile() -> Result<Vec<Pkg>> {
    let profile = config::profile_dir();
    let out = Command::new("nix-env")
    .args(["--profile", profile.to_str().unwrap(),
          "--query", "--installed", "--json"])
    .output()
    .with_context(|| "cannot query nix-env profile")?;

    if !out.status.success() { return Ok(vec![]); }
    let raw = String::from_utf8_lossy(&out.stdout);
    if raw.trim().is_empty() || raw.trim() == "{}" { return Ok(vec![]); }

    let v: serde_json::Value = serde_json::from_str(&raw)
    .unwrap_or(serde_json::json!({}));
    let mut pkgs = Vec::new();
    if let Some(obj) = v.as_object() {
        for (name, info) in obj {
            pkgs.push(Pkg {
                name: name.clone(),
                      version: info["version"].as_str().unwrap_or("?").into(),
                      attr_path: name.clone(),
                      description: info["description"].as_str().unwrap_or("").into(),
                      homepage: None, license: None,
            });
        }
    }
    pkgs.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(pkgs)
}

// ─── garbage collect ─────────────────────────────────────────────────────────

pub fn gc(task: &progress::TaskProgress) -> Result<()> {
    task.log("nix-store --gc");
    let success = progress::run_with_log(task, "nix-store", &["--gc"])?;
    if !success { return Err(anyhow!("nix-store --gc failed")); }
    Ok(())
}

// ─── generations ─────────────────────────────────────────────────────────────

pub fn list_generations() -> Result<Vec<(u32, String, bool)>> {
    let profile = config::profile_dir();
    let out = Command::new("nix-env")
    .args(["--profile", profile.to_str().unwrap(), "--list-generations"])
    .output()
    .with_context(|| "cannot list generations")?;

    let raw = String::from_utf8_lossy(&out.stdout);
    let mut gens = Vec::new();
    for line in raw.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            let num: u32 = parts[0].parse().unwrap_or(0);
            let date = parts[1].to_string();
            let current = line.contains("(current)");
            gens.push((num, date, current));
        }
    }
    Ok(gens)
}

pub fn switch_generation(gen: u32, task: &progress::TaskProgress) -> Result<()> {
    let profile = config::profile_dir();
    task.log(&format!("nix-env --profile {} --switch-generation {}", profile.display(), gen));
    let success = progress::run_with_log(task, "nix-env", &[
        "--profile", profile.to_str().unwrap(),
                                         "--switch-generation", &gen.to_string(),
    ])?;
    if !success { return Err(anyhow!("failed to switch to generation {}", gen)); }
    Ok(())
}

// ─── store du ────────────────────────────────────────────────────────────────

pub fn store_du() -> String {
    Command::new("du").args(["-sh", "/nix/store"]).output().ok()
    .and_then(|o| String::from_utf8(o.stdout).ok())
    .map(|s| s.split_whitespace().next().unwrap_or("?").to_string())
    .unwrap_or_else(|| "?".into())
}
