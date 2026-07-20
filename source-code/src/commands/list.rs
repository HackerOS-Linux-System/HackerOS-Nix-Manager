use anyhow::Result;
use crate::{nix, output, pkgdb, state};

// A default cap on how many rows a plain `hnm list` prints to the
// terminal when it's showing the *full* catalog (tens of thousands of
// packages) rather than just what's installed. `--json` always dumps
// everything, uncapped.
const DEFAULT_ALL_LIMIT: usize = 200;

pub fn run(installed_only: bool, json: bool) -> Result<()> {
    nix::ensure_nix()?;

    if installed_only {
        run_installed(json)
    } else {
        run_all(json)
    }
}

/// `hnm list -i` / `hnm list --installed` — packages HNM has actually
/// installed and is tracking in state.json. This was the *only* thing
/// `hnm list` could ever show, regardless of the flag — see
/// docs/hnm.html#changelog (v0.2).
fn run_installed(json: bool) -> Result<()> {
    // Best-effort reconciliation against the real nix-env profile before
    // we display anything, so a prior `hnm rollback` (or a package
    // installed/removed by hand with nix-env) doesn't leave `hnm list`
    // showing stale data — see state::sync_from_profile and
    // docs/hnm.html#changelog (v0.2).
    if let Ok(profile_pkgs) = nix::list_profile() {
        if let Ok((added, removed)) = state::sync_from_profile(&profile_pkgs) {
            if !json && (added > 0 || removed > 0) {
                output::dim(&format!(
                    "synced with nix-env profile ({} added, {} removed)",
                    added, removed
                ));
            }
        }
    }

    let pkgs = state::list()?;
    let st   = state::load()?;

    if json {
        println!("{}", serde_json::to_string_pretty(&pkgs)?);
        return Ok(());
    }

    output::header("Installed packages");

    if pkgs.is_empty() {
        output::dim("no packages installed via hnm yet.");
        output::dim("Run `hnm search <query>` to find packages.");
        return Ok(());
    }

    output::table_header();
    for pkg in &pkgs {
        let pin = pkg.pinned.as_ref()
            .map(|v| format!(" [pinned: {}]", v))
            .unwrap_or_default();
        let name  = format!("{}{}", pkg.name, pin);
        let desc  = pkg.description.as_deref().unwrap_or("");
        output::table_row(&name, &pkg.version, desc, true);
    }

    println!();
    output::dim(&format!("{} package(s) installed", pkgs.len()));
    if let Some(ts) = st.last_update {
        output::dim(&format!("last updated: {}", ts.format("%Y-%m-%d %H:%M UTC")));
    }
    Ok(())
}

/// Plain `hnm list` (no flag) — every package in the local nixpkgs index
/// (pkgdb.tsv, built by `hnm update`), with a ✓ next to anything HNM has
/// installed. This mode used to not exist at all: `hnm list` always
/// behaved like `hnm list -i` no matter what — see docs/hnm.html
/// #changelog (v0.2).
fn run_all(json: bool) -> Result<()> {
    if !pkgdb::db_path().exists() {
        output::warn("no package index yet — run `hnm update` first to build it");
        return Ok(());
    }

    let entries = pkgdb::all(if json { None } else { Some(DEFAULT_ALL_LIMIT) })?;
    let total = pkgdb::entry_count();
    let st = state::load()?;

    if json {
        #[derive(serde::Serialize)]
        struct Row<'a> {
            attr: &'a str,
            name: &'a str,
            version: &'a str,
            description: &'a str,
            installed: bool,
        }
        let rows: Vec<Row> = entries.iter().map(|e| Row {
            attr: &e.attr,
            name: &e.name,
            version: &e.version,
            description: &e.description,
            installed: st.installed.contains_key(&e.name),
        }).collect();
        println!("{}", serde_json::to_string_pretty(&rows)?);
        return Ok(());
    }

    output::header("Available packages");
    output::table_header();
    for e in &entries {
        let is_installed = st.installed.contains_key(&e.name);
        output::table_row(&e.name, &e.version, &e.description, is_installed);
    }

    println!();
    if total > entries.len() {
        output::dim(&format!(
            "showing {} of {} package(s) — use `hnm search <query>` to filter, or `--json` for the full list",
            entries.len(), total
        ));
    } else {
        output::dim(&format!("{} package(s) in the index", total));
    }
    output::dim("Tip: `hnm list -i` shows only packages installed via hnm.");
    Ok(())
}
