use anyhow::{Context, Result};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use crate::config::{data_dir, home};

pub fn db_path() -> PathBuf {
    data_dir().join("pkgdb.tsv")
}

/// Search the local cache. Fast, zero RAM.
pub fn search(query: &str) -> Result<Vec<(String, String, String)>> {
    let path = db_path();
    if !path.exists() {
        return Ok(vec![]);
    }
    let file = fs::File::open(&path)
        .with_context(|| format!("cannot open pkgdb at {:?}", path))?;
    let reader = BufReader::new(file);
    let q = query.to_lowercase();
    let mut results = Vec::new();
    for line in reader.lines() {
        let line = match line { Ok(l) => l, Err(_) => continue };
        let mut parts = line.splitn(3, '\t');
        let attr    = parts.next().unwrap_or("").to_string();
        let name    = parts.next().unwrap_or("").to_string();
        let version = parts.next().unwrap_or("").to_string();
        if attr.to_lowercase().contains(&q) || name.to_lowercase().contains(&q) {
            results.push((attr, name, version));
        }
    }
    Ok(results)
}

pub fn is_fresh() -> bool {
    let path = db_path();
    if !path.exists() { return false; }
    if let Ok(meta) = fs::metadata(&path) {
        if let Ok(modified) = meta.modified() {
            if let Ok(age) = modified.elapsed() {
                return age.as_secs() < 7 * 24 * 3600;
            }
        }
    }
    false
}

pub fn entry_count() -> usize {
    let path = db_path();
    if !path.exists() { return 0; }
    let Ok(file) = fs::File::open(&path) else { return 0; };
    BufReader::new(file).lines().count()
}

/// Rebuild the package database.
/// Streams nix-env -qaP output line-by-line into TSV — O(1) RAM.
pub fn rebuild_db(
    log:  impl Fn(&str),
    elog: impl Fn(&str),
) -> Result<usize> {
    use std::process::{Command, Stdio};

    let h = home();
    let path     = db_path();
    let tmp_path = path.with_extension("tsv.tmp");
    fs::create_dir_all(path.parent().unwrap())?;

    // nix-env -qaP reads packages from ~/.nix-defexpr by default.
    // We can also point it explicitly at the nixpkgs channel.
    // Use NIX_DEFEXPR to make sure it points at the right place.
    let defexpr       = h.join(".nix-defexpr");
    let nix_profile   = h.join(".nix-profile/bin");
    let nix_store_bin = std::path::Path::new("/nix/var/nix/profiles/default/bin");
    let cur_path      = std::env::var("PATH").unwrap_or_default();
    let new_path      = format!("{}:{}:{}", nix_profile.display(), nix_store_bin.display(), cur_path);

    // NIX_PATH — used by nix-env when evaluating attribute paths
    let channels_nixpkgs = h.join(".nix-defexpr/channels/nixpkgs");
    let channels_dir     = h.join(".nix-defexpr/channels");
    let nix_path = if channels_nixpkgs.exists() {
        format!("nixpkgs={}:{}", channels_nixpkgs.display(), channels_dir.display())
    } else {
        format!("{}", channels_dir.display())
    };

    log(&format!(
        "nix-env -qaP  [NIX_DEFEXPR={}, NIX_PATH={}]",
        defexpr.display(), nix_path
    ));
    log(&format!("writing to {}", tmp_path.display()));

    let mut child = Command::new("nix-env")
        .args(["-qaP"])
        .env("PATH",                 &new_path)
        .env("NIX_PATH",             &nix_path)
        .env("NIX_DEFEXPR",          &defexpr)
        .env("NIXPKGS_ALLOW_UNFREE", "1")
        .env("GC_INITIAL_HEAP_SIZE", "67108864")
        .env("GC_MAXIMUM_HEAP_SIZE", "838860800")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())   // ← show errors so we can debug
        .spawn()
        .with_context(|| "failed to spawn nix-env -qaP")?;

    // Drain stderr in a thread so it doesn't block stdout
    let stderr_handle = {
        let stderr = child.stderr.take().unwrap();
        let elog_lines: std::sync::Arc<std::sync::Mutex<Vec<String>>> =
            std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let capture = elog_lines.clone();
        let handle = std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines() {
                if let Ok(l) = line {
                    if let Ok(mut v) = capture.lock() { v.push(l); }
                }
            }
        });
        (handle, elog_lines)
    };

    let stdout = child.stdout.take().unwrap();
    let reader = BufReader::new(stdout);

    let mut out = fs::File::create(&tmp_path)
        .with_context(|| format!("cannot create {:?}", tmp_path))?;

    let mut count = 0usize;
    for line in reader.lines() {
        let line = match line { Ok(l) => l, Err(_) => continue };
        let line = line.trim().to_string();
        if line.is_empty() { continue; }

        let mut cols  = line.splitn(2, char::is_whitespace);
        let attr      = cols.next().unwrap_or("").trim();
        let name_ver  = cols.next().unwrap_or("").trim();
        let (name, version) = split_name_version(name_ver);

        writeln!(out, "{}\t{}\t{}", attr, name, version)?;
        count += 1;
        if count % 5000 == 0 {
            log(&format!("  indexed {} packages...", count));
        }
    }

    let _ = child.wait();
    let _ = stderr_handle.0.join();

    // Print any stderr lines
    if let Ok(lines) = stderr_handle.1.lock() {
        for l in lines.iter() {
            if l.contains("error:") {
                elog(l);
            } else if !l.is_empty() {
                log(l);
            }
        }
    }

    if count == 0 {
        let _ = fs::remove_file(&tmp_path);

        // Provide a more helpful error with diagnostics
        let defexpr_exists  = defexpr.exists();
        let channels_exists = channels_nixpkgs.exists();
        return Err(anyhow::anyhow!(
            "nix-env -qaP produced no output.\n  \
             NIX_DEFEXPR exists: {defexpr_exists}  (~/.nix-defexpr)\n  \
             channels/nixpkgs:   {channels_exists}  (~/.nix-defexpr/channels/nixpkgs)\n  \
             Fix: run `hnm unpack` to register the nixpkgs channel, then `hnm update` again."
        ));
    }

    // Atomically replace old db
    fs::rename(&tmp_path, &path)
        .with_context(|| "failed to rename tmp pkgdb")?;

    log(&format!("package index built: {} packages", count));
    Ok(count)
}

fn split_name_version(s: &str) -> (String, String) {
    let bytes = s.as_bytes();
    let mut split_at = None;
    for i in (1..bytes.len()).rev() {
        if bytes[i - 1] == b'-' && bytes[i].is_ascii_digit() {
            split_at = Some(i - 1);
            break;
        }
    }
    match split_at {
        Some(i) => (s[..i].to_string(), s[i + 1..].to_string()),
        None    => (s.to_string(), String::new()),
    }
}
