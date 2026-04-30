use anyhow::{anyhow, Result};
use crate::{nix, output, progress, state};

pub fn run(packages: &[String], _force: bool) -> Result<()> {
    if packages.is_empty() {
        return Err(anyhow!("usage: hnm remove <package> [<package>...]"));
    }
    nix::ensure_nix()?;
    output::header(&format!("Removing {} package(s)", packages.len()));

    let mut failed: Vec<String> = Vec::new();

    for (idx, pkg_name) in packages.iter().enumerate() {
        println!();
        output::step(
            &format!("{}/{}", idx + 1, packages.len()),
            &format!("removing  {}", pkg_name),
        );

        if !state::is_installed(pkg_name) {
            output::warn(&format!("'{}' is not installed — skipping", pkg_name));
            continue;
        }

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
    } else {
        return Err(anyhow!("failed to remove: {}", failed.join(", ")));
    }
    Ok(())
}
