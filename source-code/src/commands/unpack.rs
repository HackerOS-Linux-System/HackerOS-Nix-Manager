use anyhow::{anyhow, Result};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use crate::{config, nix, output, progress};

fn find_nix_sh() -> Option<PathBuf> {
    let home = config::home();
    [
        home.join(".nix-profile/etc/profile.d/nix.sh"),
        PathBuf::from("/nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh"),
        PathBuf::from("/etc/profile.d/nix.sh"),
        home.join(".local/state/nix/profiles/profile/etc/profile.d/nix.sh"),
    ]
    .into_iter()
    .find(|p| p.exists())
}

/// Append HNM+Nix PATH lines to shell rc files if not already present.
fn patch_shell_rc() {
    let home = config::home();
    let profile_bin = config::profile_dir().join("bin");
    let nix_sh = find_nix_sh();

    let mut lines = vec![
        String::from(""),
        String::from("# HNM — HackerOS Nix Manager"),
        format!("export PATH=\"{}:$HOME/.nix-profile/bin:$PATH\"", profile_bin.display()),
    ];
    if let Some(ref sh) = nix_sh {
        lines.push(format!("[ -f '{}' ] && . '{}'", sh.display(), sh.display()));
    } else {
        lines.push(
            "[ -f ~/.nix-profile/etc/profile.d/nix.sh ] && . ~/.nix-profile/etc/profile.d/nix.sh"
            .to_string(),
        );
    }
    lines.push(String::from(""));

    let block = lines.join("\n");
    let marker = "# HNM — HackerOS Nix Manager";

    for rc in &[".bashrc", ".zshrc"] {
        let rc_path = home.join(rc);

        // Only patch if file exists
        if !rc_path.exists() { continue; }

        let content = fs::read_to_string(&rc_path).unwrap_or_default();
        if content.contains(marker) {
            output::dim(&format!("  {} — already patched", rc_path.display()));
            continue;
        }

        match fs::OpenOptions::new().append(true).open(&rc_path) {
            Ok(mut f) => {
                let _ = f.write_all(block.as_bytes());
                output::ok(&format!("patched {}", rc_path.display()));
            }
            Err(e) => {
                output::warn(&format!("could not patch {}: {}", rc_path.display(), e));
            }
        }
    }
}

pub fn run() -> Result<()> {
    output::header("Bootstrap Nix  (hnm unpack)");

    // ── Step 1 — install Nix if missing ──────────────────────────────────────
    if nix::nix_ok() {
        output::ok("Nix is already installed — skipping download");
    } else {
        output::info("Downloading Nix installer from nixos.org...");
        output::dim("This requires internet access and may take a few minutes.");

        let task = progress::TaskProgress::new(100, "downloading & installing nix");
        task.log("sh <(curl -fsSL https://nixos.org/nix/install) --no-daemon");

        let status = Command::new("sh")
        .arg("-c")
        .arg("curl -fsSL https://nixos.org/nix/install | sh -s -- --no-daemon")
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status();

        task.inc(60);

        match status {
            Ok(s) if s.success() => {
                task.inc(40);
                task.finish_ok("Nix installed");
            }
            Ok(s) => {
                task.finish_err(&format!("installer exited with code {}", s));
                // Don't abort — nix might already be installed (daemon socket error
                // happens when nix-store is present but daemon isn't running)
                output::warn("installer returned non-zero — checking if nix is usable anyway...");
                if !nix::nix_ok() {
                    return Err(anyhow!("Nix installation failed — see output above"));
                }
                output::ok("nix-env is working despite installer error — continuing");
            }
            Err(e) => {
                task.finish_err(&e.to_string());
                return Err(anyhow!("failed to run installer: {}", e));
            }
        }
    }

    // ── Step 2 — locate / report nix.sh ──────────────────────────────────────
    match find_nix_sh() {
        Some(ref p) => output::ok(&format!("nix profile script: {}", p.display())),
        None => output::warn(
            "nix.sh not found yet — if you just installed Nix, open a new shell first",
        ),
    }

    // ── Step 3 — register nixpkgs-unstable channel ───────────────────────────
    let env_vars = nix::pub_nix_env_vars();
    {
        let task = progress::TaskProgress::new(100, "registering nixpkgs-unstable channel");
        task.log("nix-channel --add https://nixos.org/channels/nixpkgs-unstable nixpkgs");

        let add_ok = progress::run_with_log_env(
            &task,
            "nix-channel",
            &["--add", "https://nixos.org/channels/nixpkgs-unstable", "nixpkgs"],
            &env_vars,
        )?;
        task.inc(30);
        if !add_ok {
            task.warn("nix-channel --add returned non-zero (may already exist) — continuing");
        }

        task.set_msg("nix-channel --update  (may take a while)");
        task.log("nix-channel --update");
        let update_ok = progress::run_with_log_env(&task, "nix-channel", &["--update"], &env_vars)?;
        task.inc(70);

        if update_ok {
            task.finish_ok("channel updated");
        } else {
            task.finish_err("channel update failed");
            println!();
            output::warn("Could not update channel. Possible causes:");
            output::dim("  · No internet connection");
            output::dim(
                "  · DNS issue — try: echo 'nameserver 8.8.8.8' | sudo tee -a /etc/resolv.conf",
            );
            output::dim("  · Run `hnm unpack` again once connectivity is restored.");
            return Err(anyhow!("nix-channel --update failed"));
        }
    }

    // ── Step 4 — patch shell rc files ────────────────────────────────────────
    println!();
    output::info("Patching shell RC files (~/.bashrc / ~/.zshrc)...");
    patch_shell_rc();

    // ── Done ─────────────────────────────────────────────────────────────────
    println!();
    output::ok("Nix is ready!");
    println!();
    output::info("Reload your shell to apply PATH changes:");
    println!("  source ~/.zshrc   # or ~/.bashrc");
    println!();
    output::dim("Then verify with:  hnm check");
    output::dim("Build package index:  hnm update");

    Ok(())
}
