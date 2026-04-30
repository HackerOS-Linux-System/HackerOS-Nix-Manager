use anyhow::{anyhow, Result};
use crate::{nix, output, state};

pub fn run(package: &str, version: Option<&str>) -> Result<()> {
    nix::ensure_nix()?;

    if !state::is_installed(package) {
        return Err(anyhow!("'{}' is not installed — install it first with `hnm install {}`", package, package));
    }

    let pin_ver = match version {
        Some(v) => v.to_string(),
        None => {
            // Use the currently installed version
            state::get(package)
                .map(|p| p.version)
                .unwrap_or_else(|| "?".to_string())
        }
    };

    state::pin(package, &pin_ver)?;

    output::ok(&format!("pinned '{}' to version '{}'", package, pin_ver));
    output::dim("Pinned packages will not be updated by `hnm update`.");
    Ok(())
}
