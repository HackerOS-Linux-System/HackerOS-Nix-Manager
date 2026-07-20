use anyhow::{anyhow, Result};
use crate::{config, nix, output, progress, state};

pub fn run(packages: &[String], force: bool) -> Result<()> {
    if packages.is_empty() {
        return Err(anyhow!("usage: hnm remove <package> [<package>...]"));
    }
    nix::ensure_nix()?;
    output::header(&format!("Removing {} package(s)", packages.len()));

    // `--force` used to be parsed and completely ignored. It now does two
    // real things — see docs/hnm.html#changelog (v0.2):
    //   1. without it, pinned packages are skipped rather than removed
    //   2. without it, the user gets a single y/N confirmation prompt
    //      listing exactly what's about to be removed
    let st = state::load()?;
    let mut to_remove: Vec<String> = Vec::new();
    let mut skipped_pinned: Vec<String> = Vec::new();
    let mut not_installed: Vec<String> = Vec::new();

    for pkg_name in packages {
        match st.installed.get(pkg_name) {
            None => not_installed.push(pkg_name.clone()),
            Some(entry) if entry.pinned.is_some() && !force => {
                skipped_pinned.push(pkg_name.clone())
            }
            Some(_) => to_remove.push(pkg_name.clone()),
        }
    }

    for name in &not_installed {
        output::warn(&format!("'{}' is not installed — skipping", name));
    }
    for name in &skipped_pinned {
        output::warn(&format!("'{}' is pinned — use --force to remove it anyway", name));
    }

    if to_remove.is_empty() {
        output::info("nothing to remove");
        return Ok(());
    }

    if !force {
        println!();
        output::info(&format!("about to remove: {}", to_remove.join(", ")));
        if !output::confirm("Proceed?") {
            output::warn("aborted — nothing was removed");
            return Ok(());
        }
    }

    let mut failed: Vec<String> = Vec::new();

    for (idx, pkg_name) in to_remove.iter().enumerate() {
        println!();
        output::step(
            &format!("{}/{}", idx + 1, to_remove.len()),
            &format!("removing  {}", pkg_name),
        );

        let task = progress::TaskProgress::new(100, &format!("removing {}", pkg_name));
        let result = nix::remove(pkg_name, &task);
        task.inc(90);

        match result {
            Ok(_) => {
                state::remove(pkg_name)?;
                task.inc(10);
                task.finish_ok(&format!("removed {}", pkg_name));
            }
            Err(e) => {
                task.finish_err(&e.to_string());
                failed.push(pkg_name.clone());
            }
        }
    }

    println!();
    if failed.is_empty() {
        output::ok("done");

        if let Some(gen) = nix::current_generation() {
            let _ = state::set_generation(gen);
        }

        // config.hk's `auto_gc` / `max_generations` — see
        // docs/hnm.html#config. These used to be parsed into HnmConfig
        // and never read anywhere (docs/hnm.html#changelog, v0.2).
        if let Ok(cfg) = config::load() {
            if cfg.max_generations > 0 {
                let task = progress::TaskProgress::new(1, "pruning old generations");
                if let Ok(n) = nix::prune_old_generations(cfg.max_generations, &task) {
                    if n > 0 {
                        output::dim(&format!("pruned {} old generation(s) (max_generations = {})", n, cfg.max_generations));
                    }
                }
            }
            if cfg.auto_gc {
                output::info("auto_gc is enabled in config.hk — running `hnm gc`...");
                let task = progress::TaskProgress::new(100, "nix-store --gc");
                let _ = nix::gc(&task);
            }
        }
    } else {
        return Err(anyhow!("failed to remove: {}", failed.join(", ")));
    }
    Ok(())
}
