use anyhow::Result;
use std::process::Command;
use crate::{config, nix, output, state};

pub fn run() -> Result<()> {
    output::header("HNM Doctor — system diagnostics");

    let mut warnings = 0u32;
    let mut errors   = 0u32;

    // ── Nix binary ────────────────────────────────────────────────────────────
    check_bin("nix",         &["--version"], &mut errors);
    check_bin("nix-env",     &["--version"], &mut errors);
    check_bin("nix-channel", &["--version"], &mut errors);
    check_bin("nix-store",   &["--version"], &mut warnings);

    // ── Channel ───────────────────────────────────────────────────────────────
    println!();
    let chan_out = Command::new("nix-channel").arg("--list").output();
    match chan_out {
        Ok(o) if o.status.success() => {
            let raw = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if raw.is_empty() {
                output::warn("no Nix channels registered  →  run `hnm unpack`");
                warnings += 1;
            } else {
                output::ok(&format!("channels   {}", raw.replace('\n', "  |  ")));
            }
        }
        _ => {
            output::warn("could not list channels");
            warnings += 1;
        }
    }

    // ── Profile dir ───────────────────────────────────────────────────────────
    let profile = config::profile_dir();
    if profile.exists() {
        output::ok(&format!("profile    {}", profile.display()));
    } else {
        output::warn(&format!("profile dir not found: {}  (ok if no packages installed yet)", profile.display()));
    }

    // ── Nix store ─────────────────────────────────────────────────────────────
    if std::path::Path::new("/nix/store").exists() {
        let du = nix::store_du();
        output::ok(&format!("/nix/store  exists   ({})", du));
    } else {
        output::warn("/nix/store does not exist — Nix may not be installed");
        warnings += 1;
    }

    // ── State / config ────────────────────────────────────────────────────────
    println!();
    match state::load() {
        Ok(st) => output::ok(&format!("state      {} package(s) tracked", st.installed.len())),
        Err(e) => {
            output::warn(&format!("state file corrupt: {}", e));
            warnings += 1;
        }
    }

    match config::load() {
        Ok(cfg) => output::ok(&format!("config     channel = {}", cfg.nix_channel)),
        Err(e) => {
            output::warn(&format!("config error: {}", e));
            warnings += 1;
        }
    }

    // ── PATH ──────────────────────────────────────────────────────────────────
    println!();
    let path_var = std::env::var("PATH").unwrap_or_default();
    if path_var.contains(".nix-profile") {
        output::ok("PATH       contains .nix-profile");
    } else {
        output::warn("PATH does not include ~/.nix-profile/bin");
        output::dim("Add to your shell rc:  . ~/.nix-profile/etc/profile.d/nix.sh");
        warnings += 1;
    }

    // ── Summary ───────────────────────────────────────────────────────────────
    println!();
    if errors == 0 && warnings == 0 {
        output::ok("all checks passed — HNM is healthy");
    } else {
        if errors > 0 {
            output::warn(&format!("{} error(s) found", errors));
        }
        if warnings > 0 {
            output::dim(&format!("{} warning(s) — see above", warnings));
        }
    }

    Ok(())
}

fn check_bin(bin: &str, args: &[&str], counter: &mut u32) {
    match Command::new(bin).args(args).output() {
        Ok(o) if o.status.success() => {
            let ver = String::from_utf8_lossy(&o.stdout).trim().to_string();
            output::ok(&format!("{:<14} {}", bin, ver));
        }
        _ => {
            output::warn(&format!("{:<14} NOT FOUND", bin));
            *counter += 1;
        }
    }
}
