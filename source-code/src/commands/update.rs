use anyhow::Result;
use chrono::Utc;
use crate::{nix, output, pkgdb, progress, state};

pub fn run(packages: Option<&[String]>) -> Result<()> {
    nix::ensure_nix()?;
    output::header("Update HNM");

    // ── Step 1: refresh nixpkgs channel ─────────────────────────────────────
    {
        let task = progress::TaskProgress::new(100, "refreshing nixpkgs channel");
        nix::update_channel(&task)?;
        task.finish_ok("channel refreshed");
    }
    println!();

    // ── Step 2: rebuild local package index ──────────────────────────────────
    // This runs nix-env -qaP and streams it to ~/.local/share/hnm/pkgdb.tsv
    // Takes 1-2 min but only RAM usage is the current line — no OOM possible.
    {
        let task = progress::TaskProgress::new(100, "building local package index...");
        task.log("streaming nix-env -qaP → ~/.local/share/hnm/pkgdb.tsv");
        task.log("this takes 1-2 minutes on first run, ~30s on subsequent runs");

        let result = pkgdb::rebuild_db(
            |msg| task.log(msg),
                                       |msg| task.err_line(msg),
        );

        match result {
            Ok(count) => {
                task.inc(100);
                task.finish_ok(&format!("indexed {} packages", count));
            }
            Err(e) => {
                task.finish_err(&format!("{}", e));
                output::warn("package index build failed — search may not work");
                output::dim("Try: hnm unpack  (to ensure nixpkgs channel is registered)");
            }
        }
    }
    println!();

    // ── Step 3: upgrade installed packages (one at a time, safe) ─────────────
    let tracked = state::list()?;

    if tracked.is_empty() {
        output::dim("no packages installed — nothing to upgrade");
    } else {
        let to_upgrade: Vec<state::InstalledPkg> = match packages {
            Some(explicit) => tracked
            .into_iter()
            .filter(|p| explicit.iter().any(|e| *e == p.name))
            .collect(),
            None => tracked.into_iter().collect(),
        };

        let (pinned, to_upgrade): (Vec<state::InstalledPkg>, Vec<state::InstalledPkg>) =
        to_upgrade.into_iter().partition(|p| p.pinned.is_some());

        for p in &pinned {
            output::warn(&format!(
                "skipping '{}' — pinned to {}",
                p.name, p.pinned.as_deref().unwrap_or("?")
            ));
        }

        if !to_upgrade.is_empty() {
            output::info(&format!(
                "upgrading {} package(s) one at a time...",
                                  to_upgrade.len()
            ));
            println!();

            let total = to_upgrade.len();
            let mut upgraded = 0usize;
            let mut failed_pkgs: Vec<String> = Vec::new();

            for (idx, pkg) in to_upgrade.iter().enumerate() {
                output::step(
                    &format!("{}/{}", idx + 1, total),
                             &format!("upgrading  {}", pkg.name),
                );
                let task = progress::TaskProgress::new(100, &format!("nix-env -uA nixpkgs.{}", pkg.name));
                let result = nix::upgrade_one(&pkg.name, &task);
                task.inc(90);
                match result {
                    Ok(_) => {
                        task.inc(10);
                        task.finish_ok(&format!("{} upgraded", pkg.name));
                        upgraded += 1;
                    }
                    Err(e) => {
                        task.finish_err(&format!("{}", e));
                        failed_pkgs.push(pkg.name.clone());
                    }
                }
                println!();
            }

            output::header("Update summary");
            output::label("upgraded",   &upgraded.to_string());
            output::label("failed",     &failed_pkgs.len().to_string());
            if !pinned.is_empty() {
                output::label("skipped (pinned)", &pinned.len().to_string());
            }
            if !failed_pkgs.is_empty() {
                println!();
                output::warn(&format!("failed: {}", failed_pkgs.join(", ")));
            }
        }
    }

    let mut st = state::load()?;
    st.last_update = Some(Utc::now());
    state::save(&st)?;

    println!();
    output::ok("HNM is up to date");
    Ok(())
}
