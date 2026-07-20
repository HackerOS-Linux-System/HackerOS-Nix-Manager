use anyhow::{anyhow, Result};
use crate::{config, output, shellrc};

pub fn run(sub: &str, yes: bool) -> Result<()> {
    match sub {
        "activate"   => activate(yes),
        "deactivate" => deactivate(yes),
        "status"     => status(),
        other => Err(anyhow!("unknown env subcommand '{}' — use activate | deactivate | status", other)),
    }
}

// `hnm env activate/deactivate` used to only ever print instructions,
// unlike `hnm unpack` which patches shell rc files automatically. Both
// now share the same logic (src/shellrc.rs); `env` requires an explicit
// `--yes`/`-y` opt-in since — unlike the one-shot bootstrap `unpack`
// runs — this command can reasonably be called more than once. See
// docs/hnm.html#changelog (v0.2).

fn activate(yes: bool) -> Result<()> {
    output::header("Activate HNM profile");

    let profile = config::profile_dir();
    let bin_dir = profile.join("bin");

    // Resolve the actual profile path — nix-env --profile creates a symlink chain
    // ~/.hnm/profile → ~/.hnm/profile-N-link → /nix/store/...
    let resolved = std::fs::canonicalize(&profile)
        .unwrap_or_else(|_| profile.clone());
    let resolved_bin = resolved.join("bin");

    if yes {
        output::info("patching shell rc files (~/.bashrc, ~/.zshrc, ~/.profile)...");
        println!();
        let patched = shellrc::patch(&bin_dir);
        println!();
        if patched.is_empty() {
            output::info("nothing to patch — already activated, or no rc files found");
        } else {
            output::ok(&format!("patched: {}", patched.join(", ")));
            output::dim("Reload your shell (or open a new terminal) to apply.");
        }
        return Ok(());
    }

    output::info("Add the following to your ~/.bashrc or ~/.zshrc:");
    println!();
    println!("  # HNM + Nix");
    println!("  export PATH=\"{}:$PATH\"", bin_dir.display());

    // Also find and print the nix.sh sourcing line
    let nix_sh = shellrc::find_nix_sh();
    if let Some(ref sh) = nix_sh {
        println!("  [ -f '{}' ] && . '{}'", sh.display(), sh.display());
    } else {
        println!("  [ -f ~/.nix-profile/etc/profile.d/nix.sh ] && \\");
        println!("      . ~/.nix-profile/etc/profile.d/nix.sh");
    }

    println!();
    output::info("To activate for the CURRENT session only (copy & run):");
    println!();
    println!("  export PATH=\"{}:$PATH\"", bin_dir.display());
    if let Some(ref sh) = nix_sh {
        println!("  . '{}'", sh.display());
    }

    println!();
    output::dim("Tip: run `hnm env activate --yes` to patch these files automatically.");
    println!();

    // Show what's actually in the profile bin
    if resolved_bin.exists() {
        let bins: Vec<_> = std::fs::read_dir(&resolved_bin)
            .ok()
            .map(|d| d.filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().to_string())
                .collect())
            .unwrap_or_default();
        if !bins.is_empty() {
            output::dim(&format!("binaries in profile: {}", bins.join("  ")));
        } else {
            output::dim("profile/bin is empty — this is unexpected after install");
        }
    } else {
        output::warn("profile/bin does not exist yet — install a package first");
    }

    println!();
    Ok(())
}

fn deactivate(yes: bool) -> Result<()> {
    output::header("Deactivate HNM profile");
    let profile = config::profile_dir();

    if yes {
        output::info("removing the HNM block from shell rc files...");
        println!();
        let unpatched = shellrc::unpatch();
        println!();
        if unpatched.is_empty() {
            output::info("nothing to remove — no rc file had an HNM block");
        } else {
            output::ok(&format!("unpatched: {}", unpatched.join(", ")));
            output::dim("Restart your shell (or open a new terminal) to apply.");
        }
        return Ok(());
    }

    output::info("Remove this block from your ~/.bashrc or ~/.zshrc:");
    println!();
    println!("  {}", shellrc::MARKER);
    println!("  export PATH=\"{}:$PATH\"", profile.join("bin").display());
    println!("  [ -f ... ] && . ...   # nix.sh sourcing line");
    println!();
    output::dim("Tip: run `hnm env deactivate --yes` to do this automatically.");
    println!();
    output::ok("done — restart your shell to apply");
    Ok(())
}

fn status() -> Result<()> {
    output::header("HNM environment status");

    let profile = config::profile_dir();
    let bin_dir = profile.join("bin");

    output::label("profile dir", &profile.display().to_string());
    output::label("bin dir",     &bin_dir.display().to_string());

    let in_path = std::env::var("PATH")
        .map(|p| p.split(':').any(|part| {
            let p = std::path::Path::new(part);
            p == bin_dir || std::fs::canonicalize(p).map(|r| r == bin_dir).unwrap_or(false)
        }))
        .unwrap_or(false);

    output::label("in PATH", if in_path { "yes ✓" } else { "no — run `hnm env activate`" });

    let nix_profile_sourced = std::env::var("PATH")
        .map(|p| p.contains(".nix-profile"))
        .unwrap_or(false);
    output::label("nix profile", if nix_profile_sourced { "sourced ✓" } else { "not sourced" });

    // Whether a shell rc file already has the HNM block patched in.
    let home = config::home();
    let patched_files: Vec<String> = [".bashrc", ".zshrc", ".profile"]
        .into_iter()
        .filter(|rc| {
            std::fs::read_to_string(home.join(rc))
                .map(|c| c.contains(shellrc::MARKER))
                .unwrap_or(false)
        })
        .map(String::from)
        .collect();
    let patched_label = if patched_files.is_empty() {
        "none (run `hnm env activate --yes`)".to_string()
    } else {
        patched_files.join(", ")
    };
    output::label("rc files patched", &patched_label);

    // Show what's installed in the profile
    if bin_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&bin_dir) {
            let bins: Vec<_> = entries
                .filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().to_string())
                .take(20)
                .collect();
            if !bins.is_empty() {
                output::label("binaries", &bins.join(", "));
            }
        }
    }

    // Also check default nix-env profile
    let default_profile_bin = config::home().join(".nix-profile").join("bin");
    if default_profile_bin.exists() {
        output::label("~/.nix-profile/bin", "exists (may also contain packages)");
    }

    println!();
    Ok(())
}
