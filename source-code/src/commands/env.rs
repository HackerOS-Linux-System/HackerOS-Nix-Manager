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

    // Print the shell snippet — user sources it or adds to rc
    output::info("Add the following to your shell RC file (~/.bashrc or ~/.zshrc):");
    println!();
    println!("  # HNM profile");
    println!("  export PATH=\"{}:$PATH\"", bin_dir.display());
    println!("  [ -f ~/.nix-profile/etc/profile.d/nix.sh ] && \\");
    println!("      . ~/.nix-profile/etc/profile.d/nix.sh");
    println!();
    output::dim("Or to activate for the current session only, run:");
    println!("  export PATH=\"{}:$PATH\"", bin_dir.display());
    println!();
    output::ok("profile path printed — add it to your RC file to persist");
    Ok(())
}

fn deactivate() -> Result<()> {
    output::header("Deactivate HNM profile");
    let profile = config::profile_dir();
    let bin_dir = profile.join("bin");
    output::info("Remove this line from your ~/.bashrc or ~/.zshrc:");
    println!();
    println!("  export PATH=\"{}:$PATH\"", bin_dir.display());
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
    .map(|p| p.contains(bin_dir.to_str().unwrap_or("")))
    .unwrap_or(false);

    output::label("in PATH", if in_path { "yes ✓" } else { "no — run `hnm env activate`" });

    let nix_profile_sourced = std::env::var("PATH")
    .map(|p| p.contains(".nix-profile"))
    .unwrap_or(false);
    output::label("nix profile", if nix_profile_sourced { "sourced ✓" } else { "not sourced" });

    println!();
    Ok(())
}
