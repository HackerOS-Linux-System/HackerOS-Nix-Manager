use anyhow::Result;
use crate::{nix, output, progress, state};

pub fn run(query: &str, json: bool) -> Result<()> {
    nix::ensure_nix()?;

    let task = progress::TaskProgress::new(100, &format!("searching for '{}'", query));
    let results = nix::search(query, &task)?;
    task.finish_ok("done");
    println!();

    if results.is_empty() {
        output::warn(&format!("no packages found matching '{}'", query));
        return Ok(());
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&results)?);
        return Ok(());
    }

    output::header(&format!("Search  '{}'   ({} results)", query, results.len()));
    output::table_header();

    for pkg in &results {
        output::table_row(&pkg.name, &pkg.version, &pkg.description, state::is_installed(&pkg.name));
    }

    println!();
    output::dim("hnm install <package>   to install");
    output::dim("hnm info    <package>   for details");
    Ok(())
}
