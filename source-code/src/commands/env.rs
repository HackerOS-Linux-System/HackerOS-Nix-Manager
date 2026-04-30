use anyhow::{anyhow, Result};
use crate::{config, output};

pub fn run(sub: &str) -> Result<()> {
    match sub {
        "activate"   => activate(),
        "deactivate" => deactivate(),
        "status"     => status(),
        other => Err(anyhow!("unknown env subcommand '{}' — use activate | deactivate | status", other)),
    }
}

fn activate() -> Result<()> {
    output::header("Activate HNM profile");

    let profile = config::profile_dir();
    let bin_dir = profile.join("bin");

    // Resolve the actual profile path — nix-env --profile creates a symlink chain
    // ~/.hnm/profile → ~/.hnm/profile-N-link → /nix/store/...
    let resolved = std::fs::canonicalize(&profile)
        .unwrap_or_else(|_| profile.clone());
    let resolved_bin = resolved.join("bin");

    output::info("Add the following to your ~/.bashrc or ~/.zshrc:");
    println!();
    println!("  # HNM + Nix");
    println!("  export PATH=\"{}:$PATH\"", bin_dir.display());

    // Also find and print the nix.sh sourcing line
    let nix_sh = find_nix_sh();
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

    // Check if nix-env actually installed to our custom profile or default
    output::dim("");
    output::dim("NOTE: if 'steam' was installed before env activate, also check:");
    output::dim("  ~/.nix-profile/bin/steam");

    println!();
    Ok(())
}

fn deactivate() -> Result<()> {
    output::header("Deactivate HNM profile");
    let profile = config::profile_dir();
    output::info("Remove this line from your ~/.bashrc or ~/.zshrc:");
    println!();
    println!("  export PATH=\"{}:$PATH\"", profile.join("bin").display());
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

fn find_nix_sh() -> Option<std::path::PathBuf> {
    let home = config::home();
    let candidates = [
        home.join(".nix-profile/etc/profile.d/nix.sh"),
        std::path::PathBuf::from("/nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh"),
        std::path::PathBuf::from("/etc/profile.d/nix.sh"),
    ];
    candidates.into_iter().find(|p| p.exists())
}
