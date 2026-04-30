use anyhow::{anyhow, Result};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use crate::{config, nix, output, progress};

fn find_nix_sh() -> Option<PathBuf> {
    // When running as root, Nix installs to /root/.nix-profile
    // For multi-user install, the daemon script is elsewhere
    let home = config::home();
    [
        home.join(".nix-profile/etc/profile.d/nix.sh"),
        PathBuf::from("/nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh"),
        PathBuf::from("/etc/profile.d/nix.sh"),
        home.join(".local/state/nix/profiles/profile/etc/profile.d/nix.sh"),
        PathBuf::from("/root/.nix-profile/etc/profile.d/nix.sh"),
    ]
    .into_iter()
    .find(|p| p.exists())
}

fn has_bin(name: &str) -> bool {
    Command::new("which")
        .arg(name)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn is_root() -> bool {
    // Check effective UID
    unsafe { libc::geteuid() == 0 }
}

/// Patch shell rc files with HNM PATH lines
fn patch_shell_rc(profile_bin: &std::path::Path) {
    let home = config::home();
    let nix_sh = find_nix_sh();

    let nix_sh_line = match nix_sh {
        Some(ref p) => format!("[ -f '{}' ] && . '{}'", p.display(), p.display()),
        None => "[ -f ~/.nix-profile/etc/profile.d/nix.sh ] && \
                 . ~/.nix-profile/etc/profile.d/nix.sh"
            .to_string(),
    };

    let block = format!(
        "\n# HNM — HackerOS Nix Manager\nexport PATH=\"{}:$HOME/.nix-profile/bin:$PATH\"\n{}\n",
        profile_bin.display(),
        nix_sh_line,
    );
    let marker = "# HNM — HackerOS Nix Manager";

    for rc in &[".bashrc", ".zshrc", ".profile"] {
        let rc_path = home.join(rc);
        if !rc_path.exists() {
            continue;
        }
        let content = fs::read_to_string(&rc_path).unwrap_or_default();
        if content.contains(marker) {
            output::dim(&format!("  ~/{} — already patched", rc));
            continue;
        }
        match fs::OpenOptions::new().append(true).open(&rc_path) {
            Ok(mut f) => {
                let _ = f.write_all(block.as_bytes());
                output::ok(&format!("patched ~/{}", rc));
            }
            Err(e) => output::warn(&format!("could not patch ~/{}: {}", rc, e)),
        }
    }
}

/// Build the augmented PATH vec that includes nix-profile/bin
fn nix_path_env() -> Vec<(String, String)> {
    let home = config::home();
    let nix_bin       = home.join(".nix-profile/bin");
    let nix_store_bin = PathBuf::from("/nix/var/nix/profiles/default/bin");
    let cur           = std::env::var("PATH").unwrap_or_default();
    vec![
        ("PATH".into(), format!("{}:{}:{}", nix_bin.display(), nix_store_bin.display(), cur)),
        ("NIXPKGS_ALLOW_UNFREE".into(), "1".into()),
    ]
}

pub fn run() -> Result<()> {
    output::header("Bootstrap Nix  (hnm unpack)");

    // ── Root warning ─────────────────────────────────────────────────────────
    if is_root() {
        output::warn("Running as root (sudo).");
        output::dim("Nix single-user install works best as a regular user.");
        output::dim("If install fails, try: su - youruser && hnm unpack");
        println!();
    }

    // ── Step 1 — install Nix ─────────────────────────────────────────────────
    if nix::nix_ok() {
        output::ok("Nix is already installed — skipping download");
    } else {
        // Find downloader
        let dl_cmd = if has_bin("curl") {
            Some("curl -fsSL https://nixos.org/nix/install")
        } else if has_bin("wget") {
            Some("wget -qO- https://nixos.org/nix/install")
        } else {
            None
        };

        if dl_cmd.is_none() {
            output::warn("Neither curl nor wget found.");
            output::dim("Install one first:");
            output::dim("  apt-get install curl   # Debian/Ubuntu");
            output::dim("  dnf install curl       # Fedora/RHEL");
            output::dim("  pacman -S curl         # Arch");
            return Err(anyhow!("curl/wget not found"));
        }
        let dl = dl_cmd.unwrap();

        let task = progress::TaskProgress::new(100, "installing nix");

        let install_sh = if is_root() {
            // Warn about root but still try --no-daemon
            // User is responsible — we just make sure it's attempted
            task.warn("running as root — Nix single-user install may have issues");
            task.warn("recommend: run `hnm unpack` as a regular user instead");
            format!("{} | sh -s -- --no-daemon", dl)
        } else {
            format!("{} | sh -s -- --no-daemon", dl)
        };

        let status = Command::new("sh")
            .args(["-c", &install_sh])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status();

        task.inc(80);

        match status {
            Ok(s) if s.success() => {
                task.inc(20);
                task.finish_ok("Nix installed");
            }
            Ok(s) => {
                task.finish_err(&format!("installer exited with {}", s));
                output::warn("Checking if nix-env works anyway...");
                // Give the profile a moment to settle
                let _ = Command::new("sh")
                    .arg("-c")
                    .arg(". /root/.nix-profile/etc/profile.d/nix.sh 2>/dev/null || \
                          . ~/.nix-profile/etc/profile.d/nix.sh 2>/dev/null; \
                          nix-env --version")
                    .status();

                if !nix::nix_ok() {
                    println!();
                    output::warn("Root install failed. Recommended fix:");
                    output::dim("  1) Switch to a regular (non-root) user:");
                    output::dim("       su - youruser");
                    output::dim("  2) Run unpack as that user:");
                    output::dim("       hnm unpack");
                    output::dim("");
                    output::dim("If you MUST use root, install dependencies first:");
                    output::dim("  groupadd -r nixbld");
                    output::dim("  for i in $(seq 1 32); do");
                    output::dim("    useradd -g nixbld -G nixbld -M -N -r -s /sbin/nologin nixbld$i");
                    output::dim("  done");
                    output::dim("  Then run `hnm unpack` again.");
                    return Err(anyhow!("Nix installation failed"));
                }
                output::ok("nix-env works — continuing");
            }
            Err(e) => {
                task.finish_err(&e.to_string());
                return Err(anyhow!("failed to run installer: {}", e));
            }
        }
    }

    // ── Step 2 — report nix.sh location ──────────────────────────────────────
    match find_nix_sh() {
        Some(ref p) => output::ok(&format!("nix profile script: {}", p.display())),
        None        => output::warn(
            "nix.sh not found — you may need to open a new shell first",
        ),
    }

    // ── Step 3 — register nixpkgs-unstable channel ───────────────────────────
    let env = nix_path_env();
    {
        let task = progress::TaskProgress::new(100, "registering nixpkgs channel");
        task.log("nix-channel --add https://nixos.org/channels/nixpkgs-unstable nixpkgs");

        match progress::run_with_log_env(
            &task,
            "nix-channel",
            &["--add", "https://nixos.org/channels/nixpkgs-unstable", "nixpkgs"],
            &env,
        ) {
            Ok(true)  => {}
            Ok(false) => task.warn("nix-channel --add returned non-zero (may already be set)"),
            Err(e)    => {
                task.finish_err(&e.to_string());
                output::warn("nix-channel not in PATH yet.");
                output::dim("Open a NEW terminal and run `hnm unpack` again.");
                output::dim("Or source nix manually and retry:");
                output::dim("  . ~/.nix-profile/etc/profile.d/nix.sh && hnm unpack");
                return Err(anyhow!("nix-channel unavailable: {}", e));
            }
        }
        task.inc(30);

        task.set_msg("nix-channel --update");
        task.log("nix-channel --update");
        let ok = progress::run_with_log_env(&task, "nix-channel", &["--update"], &env)?;
        task.inc(70);

        if ok {
            task.finish_ok("nixpkgs-unstable channel registered");
        } else {
            task.finish_err("nix-channel --update failed");
            output::dim("Check internet connection, then run `hnm unpack` again.");
            return Err(anyhow!("nix-channel --update failed"));
        }
    }

    // ── Step 4 — patch shell rc files ────────────────────────────────────────
    println!();
    output::info("Patching shell RC files...");
    patch_shell_rc(&config::profile_dir().join("bin"));

    // ── Done ─────────────────────────────────────────────────────────────────
    println!();
    output::ok("Nix is ready!");
    println!();
    output::info("Reload your shell:");
    println!("  source ~/.bashrc    # bash");
    println!("  source ~/.zshrc     # zsh");
    println!("  exec $SHELL         # any shell");
    println!();
    output::dim("Verify:          hnm check");
    output::dim("Build pkg index: hnm update   (run once, takes ~2 min)");
    output::dim("Search:          hnm search <package>");

    Ok(())
}
