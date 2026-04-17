use anyhow::Result;
use crate::{nix, output, state};

pub fn run(installed_only: bool, json: bool) -> Result<()> {
    nix::ensure_nix()?;

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
