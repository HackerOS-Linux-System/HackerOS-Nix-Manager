use anyhow::{anyhow, Result};
use chrono::Utc;
use crate::{config, nix, output, progress, shellrc, state};

pub fn run(packages: &[String], no_env: bool) -> Result<()> {
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
                // Reuse the same `nix-env -qa` lookup for version *and*
                // description, instead of leaving description permanently
                // `None` the way installs used to — see docs/hnm.html
                // #changelog (v0.2). `hnm search` already benefits from
                // the same underlying data via pkgdb's description column.
                let info = nix::info(&bare).ok();
                let version = info.as_ref().map(|p| p.version.clone()).unwrap_or_else(|| "?".into());
                let description = info
                    .as_ref()
                    .map(|p| p.description.clone())
                    .filter(|d| !d.is_empty());

                let pkg = state::InstalledPkg {
                    name:         pkg_name.clone(),
                    version:      version.clone(),
                    attr_path:    format!("nixpkgs.{}", bare),
                    installed_at: Utc::now(),
                    pinned:       None,
                    description,
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

        if let Some(gen) = nix::current_generation() {
            let _ = state::set_generation(gen);
        }

        // config.hk's `max_generations` — see docs/hnm.html#config.
        if let Ok(cfg) = config::load() {
            if cfg.max_generations > 0 {
                let task = progress::TaskProgress::new(1, "pruning old generations");
                if let Ok(n) = nix::prune_old_generations(cfg.max_generations, &task) {
                    if n > 0 {
                        output::dim(&format!("pruned {} old generation(s) (max_generations = {})", n, cfg.max_generations));
                    }
                }
            }
        }

        // `--no-env` — skip the PATH activation hint. Meant for scripted /
        // non-interactive installs where the caller already manages PATH
        // itself (e.g. inside another packaging step). Previously this
        // flag was parsed but silently ignored — see docs/hnm.html
        // #changelog (v0.2).
        if !no_env {
            println!();
            let profile_bin = config::profile_dir().join("bin");
            let default_bin = config::home().join(".nix-profile").join("bin");

            output::info("To use installed packages, add to your ~/.zshrc or ~/.bashrc:");
            println!();
            println!("  export PATH=\"{}:$PATH\"", profile_bin.display());

            match shellrc::find_nix_sh() {
                Some(sh) => println!("  . '{}'", sh.display()),
                None => {
                    println!("  [ -f ~/.nix-profile/etc/profile.d/nix.sh ] && \\");
                    println!("      . ~/.nix-profile/etc/profile.d/nix.sh");
                }
            }

            println!();
            output::dim("Then reload: source ~/.zshrc  (or open a new terminal)");
            output::dim("Tip: `hnm env activate --yes` patches these files for you automatically.");

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
        }
    } else {
        output::warn(&format!("{} package(s) failed", failed.len()));
        return Err(anyhow!("failed: {}", failed.join(", ")));
    }

    Ok(())
}
