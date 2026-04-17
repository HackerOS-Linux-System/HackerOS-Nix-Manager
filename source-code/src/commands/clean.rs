use anyhow::Result;
use std::fs;
use crate::{config, output};

pub fn run() -> Result<()> {
    output::header("Clean HNM cache");

    let data_dir = config::data_dir();
    let cache_dir = config::home().join(".cache").join("hnm");
    let nix_cache  = config::home().join(".cache").join("nix");

    let mut freed = 0u64;

    // HNM cache
    if cache_dir.exists() {
        let size = dir_size(&cache_dir);
        fs::remove_dir_all(&cache_dir)?;
        freed += size;
        output::ok(&format!("removed {}  ({} bytes)", cache_dir.display(), size));
    } else {
        output::dim(&format!("{} — nothing to clean", cache_dir.display()));
    }

    // Nix evaluation cache (safe to remove)
    let nix_eval_cache = config::home().join(".cache").join("nix").join("eval-cache-v4");
    if nix_eval_cache.exists() {
        let size = dir_size(&nix_eval_cache);
        fs::remove_dir_all(&nix_eval_cache)?;
        freed += size;
        output::ok(&format!("removed nix eval cache  ({} bytes)", size));
    }

    println!();
    if freed > 0 {
        output::ok(&format!("freed {} bytes ({:.1} MB)", freed, freed as f64 / 1_048_576.0));
    } else {
        output::ok("nothing to clean — cache is already empty");
    }

    output::dim("To free Nix store space, run:  hnm gc");
    Ok(())
}

fn dir_size(path: &std::path::Path) -> u64 {
    walkdir_size(path)
}

fn walkdir_size(path: &std::path::Path) -> u64 {
    if path.is_file() {
        return path.metadata().map(|m| m.len()).unwrap_or(0);
    }
    let mut total = 0u64;
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            total += walkdir_size(&entry.path());
        }
    }
    total
}
