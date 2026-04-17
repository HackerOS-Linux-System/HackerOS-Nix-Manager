use anyhow::{anyhow, Result};
use chrono::Utc;
use crate::{nix, output, progress, state};

pub fn run(packages: &[String], _no_env: bool) -> Result<()> {
    if packages.is_empty() {
        return Err(anyhow!("usage: hnm install <package> [<package>...]"));
    }

    // Auto-install Nix if missing
    nix::ensure_nix()?;

    output::header(&format!("Installing {} package(s)", packages.len()));

    let mut failed: Vec<String> = Vec::new();

    for (idx, pkg_name) in packages.iter().enumerate() {
        println!();
        output::step(
            &format!("{}/{}", idx + 1, packages.len()),
                     &format!("installing  {}", pkg_name),
        );

        if state::is_installed(pkg_name) {
            output::warn(&format!("'{}' is already installed — skipping", pkg_name));
            continue;
        }

        // 4 steps: resolve → fetch → unpack → link
        let task = progress::TaskProgress::new(100, &format!("installing {}", pkg_name));

        // Try to resolve info first (best-effort)
        task.set_msg("resolving package...");
        task.log(&format!("nix eval nixpkgs#{}.version", pkg_name));
        let info = nix::info(pkg_name);
        let (attr_path, version, description) = match &info {
            Ok(p) => (format!("nixpkgs#{}", p.name), p.version.clone(), p.description.clone()),
            Err(_) => (format!("nixpkgs#{}", pkg_name), "?".into(), String::new()),
        };
        task.inc(10);

        task.set_msg("fetching from nixpkgs...");
        let result = nix::install(&attr_path, &task);
        task.inc(80);

        match result {
            Ok(_) => {
                let pkg = state::InstalledPkg {
                    name: pkg_name.clone(),
                    version: version.clone(),
                    attr_path: attr_path.clone(),
                    installed_at: Utc::now(),
                    pinned: None,
                    description: if description.is_empty() { None } else { Some(description) },
                };
                state::add(pkg)?;
                task.inc(10);
                task.finish_ok(&format!("{} {}", pkg_name, version));
            }
            Err(e) => {
                task.finish_err(&e.to_string());
                failed.push(pkg_name.clone());
            }
        }
    }

    println!();
    if failed.is_empty() {
        output::ok("all packages installed successfully");
        output::dim("Binaries are available via the HNM profile.");
        output::dim("If not in PATH, run:  hnm env activate");
    } else {
        output::warn(&format!("{} package(s) failed", failed.len()));
        return Err(anyhow!("some packages could not be installed: {}", failed.join(", ")));
    }

    Ok(())
}
