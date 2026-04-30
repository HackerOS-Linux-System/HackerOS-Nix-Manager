use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::io::BufRead;
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
    Command::new("nix-env").arg("--version")
        .stdout(Stdio::null()).stderr(Stdio::null())
        .status().map(|s| s.success()).unwrap_or(false)
}

pub fn ensure_nix() -> Result<()> {
    if nix_ok() { return Ok(()); }
    output::warn("Nix is not installed — bootstrapping automatically...");
    crate::commands::unpack::run()
}

// ─── env setup ───────────────────────────────────────────────────────────────

/// Returns env vars needed so nix-env can find nixpkgs.
/// KEY INSIGHT: GC_INITIAL_HEAP_SIZE limits the Boehm GC heap used by
/// the Nix evaluator. Default is 384 MiB — we keep it low to avoid OOM.
pub fn pub_nix_env_vars() -> Vec<(String, String)> {
    let home = config::home();

    // Build PATH: ~/.nix-profile/bin first, then /nix/.../default/bin
    let nix_profile_bin = home.join(".nix-profile/bin");
    let nix_default_bin = std::path::Path::new("/nix/var/nix/profiles/default/bin");
    let cur_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!(
        "{}:{}:{}",
        nix_profile_bin.display(),
        nix_default_bin.display(),
        cur_path,
    );

    // NIX_PATH: where nix-env -iA nixpkgs.X resolves nixpkgs
    // Check both the channels dir and the legacy .nix-defexpr path
    let channels_nixpkgs = home.join(".nix-defexpr/channels/nixpkgs");
    let channels_dir     = home.join(".nix-defexpr/channels");
    let nix_path = if channels_nixpkgs.exists() {
        format!("nixpkgs={}:{}", channels_nixpkgs.display(), channels_dir.display())
    } else {
        // fallback: let nix figure it out from channels dir
        format!("nixpkgs={}", channels_dir.display())
    };

    vec![
        ("PATH".into(),                 new_path),
        ("NIX_PATH".into(),             nix_path),
        // Allow unfree packages (steam, vscode, etc.) — user can opt out via config
        ("NIXPKGS_ALLOW_UNFREE".into(), "1".into()),
        // Boehm GC: start with 64 MiB heap → GC more often, use less peak RAM
        ("GC_INITIAL_HEAP_SIZE".into(), "67108864".into()),
        // Don't let GC heap grow unbounded — cap at 800 MiB
        ("GC_MAXIMUM_HEAP_SIZE".into(), "838860800".into()),
    ]
}

fn nix_env_vars() -> Vec<(String, String)> {
    pub_nix_env_vars()
}

/// Wrap a command with systemd-run memory limit if systemd is available.
/// Falls back to plain command if systemd-run not present.
/// Returns (program, args_prefix) to prepend to actual nix-env call.
fn memory_wrapper() -> Option<(String, Vec<String>)> {
    // Check if systemd-run is available
    let ok = Command::new("systemd-run")
        .args(["--version"])
        .stdout(Stdio::null()).stderr(Stdio::null())
        .status().map(|s| s.success()).unwrap_or(false);
    if !ok { return None; }

    // Use --scope (not --service) so it runs in current session,
    // MemoryMax=2G hard limit, MemorySwapMax=0 disables swap growth
    Some((
        "systemd-run".into(),
        vec![
            "--scope".into(),
            "--user".into(),
            "-p".into(), "MemoryMax=2G".into(),
            "-p".into(), "MemorySwapMax=512M".into(),
            "--".into(),
        ],
    ))
}

/// Build a Command that runs nix-env safely:
/// - proper NIX_PATH / GC env vars
/// - systemd-run memory wrapper if available
/// - ulimit -v fallback via sh -c if no systemd
fn safe_nix_env_cmd(nix_args: &[&str]) -> Command {
    let env_vars = nix_env_vars();

    if let Some((wrapper_bin, wrapper_args)) = memory_wrapper() {
        let mut cmd = Command::new(&wrapper_bin);
        cmd.args(&wrapper_args);
        cmd.arg("nix-env");
        cmd.args(nix_args);
        cmd.envs(env_vars);
        return cmd;
    }

    // No systemd-run: use sh -c with ulimit -v 2G
    // Virtual memory limit — prevents nix evaluator from mapping huge amounts
    let nix_args_str: Vec<String> = nix_args.iter().map(|s| {
        // shell-quote each arg
        format!("'{}'", s.replace('\'', "'\\''"))
    }).collect();
    let cmd_str = format!(
        "ulimit -v 2097152; exec nix-env {}",
        nix_args_str.join(" ")
    );
    let mut cmd = Command::new("sh");
    cmd.args(["-c", &cmd_str]);
    cmd.envs(env_vars);
    cmd
}

// ─── search ──────────────────────────────────────────────────────────────────
//
// Search uses the LOCAL package database (~/.local/share/hnm/pkgdb.tsv).
// This file is built once during `hnm update` by streaming nix-env -qaP into
// a TSV file. Searching it is instantaneous and uses zero extra RAM.
//
// If the db doesn't exist yet, we tell the user to run `hnm update` first.

pub fn search(query: &str, task: &progress::TaskProgress) -> Result<Vec<Pkg>> {
    use crate::pkgdb;

    let db_path = pkgdb::db_path();

    if !db_path.exists() {
        task.warn("package index not built yet");
        task.warn("run `hnm update` first to build the local package index");
        task.log("(this takes ~1-2 min but only needs to run once per update)");
        return Ok(vec![]);
    }

    let count = pkgdb::entry_count();
    task.log(&format!("searching local index ({} packages)...", count));
    task.set_msg(&format!("searching {} packages", count));

    let results = pkgdb::search(query)
        .with_context(|| "failed to search local package index")?;

    let pkgs: Vec<Pkg> = results.into_iter().map(|(attr, name, version): (String, String, String)| {
        let short = attr.strip_prefix("nixpkgs.").unwrap_or(&attr).to_string();
        Pkg {
            name:        short,
            version,
            attr_path:   attr,
            description: String::new(),
            homepage:    None,
            license:     None,
        }
    }).collect();

    task.inc(100);
    Ok(pkgs)
}
fn split_name_version(s: &str) -> (String, String) {
    let bytes = s.as_bytes();
    let mut split_at = None;
    for i in (1..bytes.len()).rev() {
        if bytes[i - 1] == b'-' && bytes[i].is_ascii_digit() {
            split_at = Some(i - 1);
            break;
        }
    }
    match split_at {
        Some(i) => (s[..i].to_string(), s[i + 1..].to_string()),
        None    => (s.to_string(), String::new()),
    }
}

// ─── nixpkgs config ───────────────────────────────────────────────────────────

/// Ensure ~/.config/nixpkgs/config.nix exists with allowUnfree = true.
/// This is required for packages like steam, vscode, discord etc.
/// Idempotent — only creates/patches if not already set.
pub fn ensure_nixpkgs_config() {
    let config_dir  = config::home().join(".config").join("nixpkgs");
    let config_file = config_dir.join("config.nix");

    // If already exists and contains allowUnfree, leave it alone
    if config_file.exists() {
        if let Ok(contents) = std::fs::read_to_string(&config_file) {
            if contents.contains("allowUnfree") {
                return; // already configured
            }
        }
    }

    // Create directory if needed
    let _ = std::fs::create_dir_all(&config_dir);

    let nix_config = "{ allowUnfree = true; }\n";
    let _ = std::fs::write(&config_file, nix_config);
}

// ─── install ─────────────────────────────────────────────────────────────────
//
// IMPORTANT: use -iA nixpkgs.steam (attribute path), NOT -i steam (name).
// -iA evaluates only ONE attribute → small memory footprint.
// Plain -i without -A scans the entire nixpkgs → OOM.

pub fn install(bare_name: &str, task: &progress::TaskProgress) -> Result<()> {
    let profile = config::profile_dir();

    // Ensure profile parent exists
    if let Some(p) = profile.parent() { std::fs::create_dir_all(p).ok(); }

    let attr = format!("nixpkgs.{}", bare_name);
    task.log(&format!("nix-env --profile {} -iA {}  [GC_INITIAL_HEAP_SIZE=64M]",
        profile.display(), attr));

    let profile_str = profile.to_str().unwrap().to_string();
    let ok = progress::run_cmd_log(
        task,
        safe_nix_env_cmd(&["--profile", &profile_str, "-iA", &attr]),
    )?;

    if !ok {
        return Err(anyhow!(
            "nix-env -iA {} failed — check that the package name is correct.\n  \
             Tip: run `hnm search {}` first to find the exact attribute path",
            attr, bare_name
        ));
    }
    Ok(())
}

// ─── remove ──────────────────────────────────────────────────────────────────

pub fn remove(pkg_name: &str, task: &progress::TaskProgress) -> Result<()> {
    let profile = config::profile_dir();
    task.log(&format!("nix-env --profile {} --uninstall {}", profile.display(), pkg_name));

    let profile_str = profile.to_str().unwrap().to_string();
    let ok = progress::run_cmd_log(
        task,
        safe_nix_env_cmd(&["--profile", &profile_str, "--uninstall", pkg_name]),
    )?;

    if !ok {
        return Err(anyhow!("nix-env --uninstall failed for '{}'", pkg_name));
    }
    Ok(())
}

// ─── update ──────────────────────────────────────────────────────────────────

pub fn update_channel(task: &progress::TaskProgress) -> Result<()> {
    let cfg = config::load()?;
    task.log(&format!("nix-channel --add {} nixpkgs", cfg.nix_channel));
    let _ = Command::new("nix-channel")
        .args(["--add", &cfg.nix_channel, "nixpkgs"])
        .envs(nix_env_vars())
        .status();

    task.set_msg("nix-channel --update");
    task.log("nix-channel --update");
    let ok = progress::run_cmd_log(
        task,
        {
            let mut c = Command::new("nix-channel");
            c.arg("--update").envs(nix_env_vars());
            c
        },
    )?;
    if !ok { return Err(anyhow!("nix-channel --update failed")); }
    Ok(())
}

/// Upgrade a single package by attribute.
/// Uses -iA which is O(1) in nixpkgs size — safe for low-RAM machines.
pub fn upgrade_one(pkg_name: &str, task: &progress::TaskProgress) -> Result<bool> {
    let profile = config::profile_dir();
    let attr = format!("nixpkgs.{}", pkg_name);
    let profile_str = profile.to_str().unwrap().to_string();

    task.log(&format!("nix-env --profile {} -iA {}  [upgrade]", profile.display(), attr));

    let ok = progress::run_cmd_log(
        task,
        safe_nix_env_cmd(&["--profile", &profile_str, "-iA", &attr]),
    )?;
    Ok(ok)
}

// ─── info ────────────────────────────────────────────────────────────────────

pub fn info(name: &str) -> Result<Pkg> {
    // Query a single attribute with -iA — low memory
    let attr = format!("nixpkgs.{}", name);
    let env  = nix_env_vars();

    // nix-env --query --available -A nixpkgs.steam --json
    let out = Command::new("nix-env")
        .args(["--query", "--available", "-A", &attr, "--json"])
        .envs(env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
        .output()
        .with_context(|| format!("cannot query info for '{name}'"))?;

    if out.status.success() && !out.stdout.is_empty() {
        let raw = String::from_utf8_lossy(&out.stdout);
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) {
            if let Some(obj) = v.as_object() {
                if let Some((attr_key, info)) = obj.iter().next() {
                    return Ok(Pkg {
                        name: info["pname"].as_str()
                            .or_else(|| info["name"].as_str())
                            .unwrap_or(name).to_string(),
                        version: info["version"].as_str().unwrap_or("?").to_string(),
                        attr_path: attr_key.clone(),
                        description: info["meta"]["description"].as_str()
                            .or_else(|| info["description"].as_str())
                            .unwrap_or("").to_string(),
                        homepage: info["meta"]["homepage"].as_str().map(String::from),
                        license: info["meta"]["license"]["spdxId"].as_str()
                            .or_else(|| info["meta"]["license"].as_str()).map(String::from),
                    });
                }
            }
        }
    }

    Err(anyhow!("package '{}' not found — try `hnm search {}`", name, name))
}

// ─── list installed ──────────────────────────────────────────────────────────

pub fn list_profile() -> Result<Vec<Pkg>> {
    let profile = config::profile_dir();
    let out = Command::new("nix-env")
        .args(["--profile", profile.to_str().unwrap(),
               "--query", "--installed", "--json"])
        .envs(nix_env_vars())
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
                description: info["meta"]["description"].as_str()
                    .or_else(|| info["description"].as_str()).unwrap_or("").into(),
                homepage: None, license: None,
            });
        }
    }
    pkgs.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(pkgs)
}

// ─── gc ──────────────────────────────────────────────────────────────────────

pub fn gc(task: &progress::TaskProgress) -> Result<()> {
    task.log("nix-store --gc");
    let mut cmd = Command::new("nix-store");
    cmd.arg("--gc").envs(nix_env_vars());
    let ok = progress::run_cmd_log(task, cmd)?;
    if !ok { return Err(anyhow!("nix-store --gc failed")); }
    Ok(())
}

// ─── generations ─────────────────────────────────────────────────────────────

pub fn list_generations() -> Result<Vec<(u32, String, bool)>> {
    let profile = config::profile_dir();
    let out = Command::new("nix-env")
        .args(["--profile", profile.to_str().unwrap(), "--list-generations"])
        .envs(nix_env_vars())
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
    let profile_str = profile.to_str().unwrap().to_string();
    let ok = progress::run_cmd_log(
        task,
        safe_nix_env_cmd(&["--profile", &profile_str, "--switch-generation", &gen.to_string()]),
    )?;
    if !ok { return Err(anyhow!("failed to switch to generation {}", gen)); }
    Ok(())
}

// ─── store du ────────────────────────────────────────────────────────────────

pub fn store_du() -> String {
    Command::new("du").args(["-sh", "/nix/store"]).output().ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.split_whitespace().next().unwrap_or("?").to_string())
        .unwrap_or_else(|| "?".into())
}
