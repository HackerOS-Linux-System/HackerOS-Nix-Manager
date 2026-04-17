use anyhow::{anyhow, Result};
use crate::{nix, output, state};

pub fn run(package: &str) -> Result<()> {
    nix::ensure_nix()?;

    if !state::is_installed(package) {
        return Err(anyhow!("'{}' is not installed", package));
    }

    state::unpin(package)?;
    output::ok(&format!("'{}' is now unpinned — it will be updated by `hnm update`", package));
    Ok(())
}
