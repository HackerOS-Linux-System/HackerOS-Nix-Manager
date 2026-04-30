use anyhow::{anyhow, Result};
use chrono::Utc;
use crate::{config, nix, output, progress, state};

pub fn run(packages: &[String], _no_env: bool) -> Result<()> {
    if packages.is_empty() {
        return Err(anyhow!("usage: hnm install <package> [<package>...]"));
    }

    nix::ensure_nix()?;
    nix::ensure_nixpkgs_config();
    output::header(&format!("Installing {} package(s)", packages.len()));

    let mut failed: Vec<String> = Vec::new();

    for (idx, pkg_name) in packages.iter().enumerate() {
        println!();
        output::step(
            &format!("{}/{}", idx + 1, packages.len()),
            &format!("installing  {}", pkg_name),
        );

        if state::is_installed(pkg_name) {
            output::warn(&format!("'{}' is already installed — skipping", pkg_name));
            continue;
        }

        let bare = pkg_name
            .strip_prefix("nixpkgs.")
            .or_else(|| pkg_name.strip_prefix("nixpkgs#"))
            .unwrap_or(pkg_name)
            .to_string();

        let task = progress::TaskProgress::new(100, &format!("nix-env -iA nixpkgs.{}", bare));
        task.log(&format!("nix-env -iA nixpkgs.{}  [NIXPKGS_ALLOW_UNFREE=1]", bare));
        task.set_msg(&format!("installing nixpkgs.{} ...", bare));

        let result = nix::install(&bare, &task);
        task.inc(90);

        match result {
            Ok(_) => {
                let version = nix::info(&bare)
                    .map(|p| p.version)
                    .unwrap_or_else(|_| "?".into());

                let pkg = state::InstalledPkg {
                    name:         pkg_name.clone(),
                    version:      version.clone(),
                    attr_path:    format!("nixpkgs.{}", bare),
                    installed_at: Utc::now(),
                    pinned:       None,
                    description:  None,
                };
                state::add(pkg)?;
                task.inc(10);
                task.finish_ok(&format!("{} {}", pkg_name, version));
            }
            Err(e) => {
                task.finish_err(&format!("{}", e));
                failed.push(pkg_name.clone());
            }
        }
    }

    println!();
    if failed.is_empty() {
        output::ok("all packages installed successfully");
        println!();
        // Show PATH instructions clearly
        let profile_bin = config::profile_dir().join("bin");
        let default_bin = config::home().join(".nix-profile").join("bin");

        output::info("To use installed packages, add to your ~/.zshrc or ~/.bashrc:");
        println!();
        println!("  export PATH=\"{}:$PATH\"", profile_bin.display());

        // Find nix.sh
        let nix_sh = find_nix_sh();
        if let Some(sh) = nix_sh {
            println!("  . '{}'", sh.display());
        } else {
            println!("  [ -f ~/.nix-profile/etc/profile.d/nix.sh ] && \\");
            println!("      . ~/.nix-profile/etc/profile.d/nix.sh");
        }

        println!();
        output::dim("Then reload: source ~/.zshrc  (or open a new terminal)");

        // Quick check — if default nix profile has the binary, mention it
        for pkg_name in packages {
            let bare = pkg_name
                .strip_prefix("nixpkgs.")
                .or_else(|| pkg_name.strip_prefix("nixpkgs#"))
                .unwrap_or(pkg_name);
            let in_hnm_profile   = profile_bin.join(bare).exists();
            let in_default_profile = default_bin.join(bare).exists();
            if in_hnm_profile {
                output::dim(&format!("  found: {}", profile_bin.join(bare).display()));
            } else if in_default_profile {
                output::dim(&format!("  found in default profile: {}", default_bin.join(bare).display()));
                output::dim(&format!("  also add: export PATH=\"{}:$PATH\"", default_bin.display()));
            }
        }
    } else {
        output::warn(&format!("{} package(s) failed", failed.len()));
        return Err(anyhow!("failed: {}", failed.join(", ")));
    }

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
