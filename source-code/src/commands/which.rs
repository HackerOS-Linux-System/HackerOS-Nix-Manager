use anyhow::{anyhow, Result};
use std::process::Command;
use crate::{config, output, state};

pub fn run(package: &str) -> Result<()> {
    // Try `which` in the HNM profile bin dir first
    let profile_bin = config::profile_dir().join("bin").join(package);

    if profile_bin.exists() {
        output::ok(&format!("{}", profile_bin.display()));
        return Ok(());
    }

    // Fallback: `which` from PATH
    let out = Command::new("which").arg(package).output();
    match out {
        Ok(o) if o.status.success() => {
            let path = String::from_utf8_lossy(&o.stdout).trim().to_string();
            output::ok(&path);
        }
        _ => {
            // Not in profile bin or PATH — maybe installed as a different binary name
            if state::is_installed(package) {
                let profile = config::profile_dir();
                output::warn(&format!(
                    "'{}' is installed but its binary was not found in {}",
                    package,
                    profile.join("bin").display()
                ));
                output::dim("The package may provide binaries under different names.");
                output::dim(&format!("Check: ls {}",
                                     profile.join("bin").display()));
            } else {
                return Err(anyhow!("'{}' is not installed — run `hnm install {}`", package, package));
            }
        }
    }
    Ok(())
}
