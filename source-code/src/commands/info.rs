use anyhow::Result;
use crate::{nix, output, state};

pub fn run(package: &str) -> Result<()> {
    nix::ensure_nix()?;
    output::header(&format!("Package info — {}", package));

    let pkg = nix::info(package)?;

    output::label("name",        &pkg.name);
    output::label("version",     &pkg.version);
    output::label("attr",        &pkg.attr_path);
    if !pkg.description.is_empty() {
        output::label("description", &pkg.description);
    }
    if let Some(hp)  = &pkg.homepage { output::label("homepage", hp); }
    if let Some(lic) = &pkg.license  { output::label("license",  lic); }

    let installed = state::is_installed(package);
    output::label("installed", if installed { "yes ✓" } else { "no" });

    if let Some(p) = state::get(package) {
        output::label("installed at", &p.installed_at.format("%Y-%m-%d %H:%M UTC").to_string());
        if let Some(pin) = &p.pinned {
            output::label("pinned to", pin);
        }
    }

    println!();
    if !installed {
        output::dim(&format!("hnm install {}   to install", package));
    }
    Ok(())
}
