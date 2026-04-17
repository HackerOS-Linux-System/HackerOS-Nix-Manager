use anyhow::{anyhow, Result};
use std::process::{Command, Stdio};
use crate::{nix, output, progress};

pub fn run() -> Result<()> {
    output::header("Bootstrap Nix  (hnm unpack)");

    // ── Step 1 — install Nix ────────────────────────────────────────────────
    if nix::nix_ok() {
        output::ok("Nix is already installed — skipping download");
    } else {
        output::info("Downloading Nix installer from nixos.org...");

        let task = progress::TaskProgress::new(100, "downloading nix installer");
        task.log("curl -L https://nixos.org/nix/install | sh --no-daemon");

        // We pipe through sh so we stream the whole installer output
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
                task.finish_err(&format!("installer exited with {}", s));
                return Err(anyhow!("Nix installation failed"));
            }
            Err(e) => {
                task.finish_err(&e.to_string());
                return Err(anyhow!("failed to run installer: {}", e));
            }
        }
    }

    // ── Step 2 — source nix profile ─────────────────────────────────────────
    output::info("Sourcing ~/.nix-profile/etc/profile.d/nix.sh ...");
    let _ = Command::new("sh")
    .arg("-c")
    .arg(". ~/.nix-profile/etc/profile.d/nix.sh")
    .status();

    // ── Step 3 — add nixpkgs-unstable channel ───────────────────────────────
    {
        let task = progress::TaskProgress::new(100, "adding nixpkgs-unstable channel");
        task.log("nix-channel --add https://nixos.org/channels/nixpkgs-unstable nixpkgs");

        let ok = progress::run_with_log(&task, "nix-channel", &[
            "--add",
            "https://nixos.org/channels/nixpkgs-unstable",
            "nixpkgs",
        ])?;
        task.inc(50);

        if !ok {
            task.warn("nix-channel --add returned non-zero — continuing anyway");
        }

        // ── Step 4 — nix-channel --update ───────────────────────────────────
        task.set_msg("nix-channel --update  (this may take a while)");
        task.log("nix-channel --update");

        let ok2 = progress::run_with_log(&task, "nix-channel", &["--update"])?;
        task.inc(50);

        if ok2 {
            task.finish_ok("channel updated");
        } else {
            task.finish_err("channel update failed");
            return Err(anyhow!("nix-channel --update failed"));
        }
    }

    println!();
    output::ok("Nix is ready!");
    output::dim("Add the following line to your ~/.bashrc or ~/.zshrc to persist the PATH:");
    output::dim("  . ~/.nix-profile/etc/profile.d/nix.sh");
    output::dim("Then restart your shell or run:  source ~/.nix-profile/etc/profile.d/nix.sh");
    println!();
    output::dim("Run `hnm check` to verify the installation.");

    Ok(())
}
