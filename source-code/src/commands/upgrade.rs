use anyhow::{anyhow, Result};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use crate::{config, output, progress};

// ─── version file paths ───────────────────────────────────────────────────────

fn local_version_file() -> PathBuf {
    config::home()
    .join(".hackeros")
    .join("hnm")
    .join("version.hacker")
}

fn tmp_version_file() -> PathBuf {
    std::env::temp_dir().join(".hnm_upstream_version.hacker")
}

const UPSTREAM_VERSION_URL: &str =
"https://raw.githubusercontent.com/HackerOS-Linux-System/HackerOS-Nix-Manager/main/version.hacker";

    const UPSTREAM_RELEASE_URL: &str =
    "https://github.com/HackerOS-Linux-System/HackerOS-Nix-Manager/releases/download";

    // ─── version.hacker parser ────────────────────────────────────────────────────
    // Format:
    // [
    //   0.1
    // ]

    fn parse_version(content: &str) -> Option<String> {
        for line in content.lines() {
            let t = line.trim().trim_matches('[').trim_matches(']').trim();
            if t.is_empty() { continue; }
            // Validate it looks like a version  (digits and dots only)
            if t.chars().all(|c| c.is_ascii_digit() || c == '.') {
                return Some(t.to_string());
            }
        }
        None
    }

    fn version_greater(upstream: &str, local: &str) -> bool {
        let parse = |s: &str| -> Vec<u64> {
            s.split('.').map(|p| p.parse().unwrap_or(0)).collect()
        };
        let u = parse(upstream);
        let l = parse(local);
        let max_len = u.len().max(l.len());
        for i in 0..max_len {
            let ui = u.get(i).copied().unwrap_or(0);
            let li = l.get(i).copied().unwrap_or(0);
            if ui > li { return true; }
            if ui < li { return false; }
        }
        false
    }

    // ─── network helpers ──────────────────────────────────────────────────────────

    fn curl_download(url: &str, dest: &PathBuf, task: &progress::TaskProgress) -> Result<()> {
        task.log(&format!("curl -fsSL {} -o {}", url, dest.display()));
        let status = Command::new("curl")
        .args(["-fsSL", url, "-o", dest.to_str().unwrap()])
        .status()
        .map_err(|e| anyhow!("curl not found: {}", e))?;
        if !status.success() {
            return Err(anyhow!("curl failed downloading {}", url));
        }
        Ok(())
    }

    // ─── main upgrade logic ───────────────────────────────────────────────────────

    pub fn run() -> Result<()> {
        output::header("HNM Self-Upgrade");

        // ── 1. Read local version ─────────────────────────────────────────────────
        let local_ver_path = local_version_file();
        let local_version = if local_ver_path.exists() {
            let content = fs::read_to_string(&local_ver_path)
            .map_err(|e| anyhow!("cannot read {}: {}", local_ver_path.display(), e))?;
            parse_version(&content)
            .ok_or_else(|| anyhow!("cannot parse local version file at {}", local_ver_path.display()))?
        } else {
            // Create with current cargo version as default
            let ver = env!("CARGO_PKG_VERSION").to_string();
            output::warn(&format!(
                "local version file not found at {} — creating with version {}",
                local_ver_path.display(), ver
            ));
            if let Some(parent) = local_ver_path.parent() {
                fs::create_dir_all(parent)?;
            }
            let content = format!("[\n  {}\n]\n", ver);
            fs::write(&local_ver_path, &content)?;
            ver
        };

        output::label("current version", &local_version);

        // ── 2. Download upstream version.hacker ──────────────────────────────────
        {
            let task = progress::TaskProgress::new(100, "checking for updates...");
            task.log(&format!("fetching {}", UPSTREAM_VERSION_URL));

            let tmp = tmp_version_file();
            let result = curl_download(UPSTREAM_VERSION_URL, &tmp, &task);
            task.inc(50);

            match result {
                Ok(_) => task.finish_ok("version info fetched"),
                Err(e) => {
                    task.finish_err(&format!("{}", e));
                    output::warn("Could not reach GitHub — check your internet connection.");
                    return Err(anyhow!("cannot check for updates: {}", e));
                }
            }
        }

        // ── 3. Parse upstream version ─────────────────────────────────────────────
        let tmp = tmp_version_file();
        let upstream_content = fs::read_to_string(&tmp)
        .map_err(|e| anyhow!("cannot read downloaded version file: {}", e))?;
        let _ = fs::remove_file(&tmp);

        let upstream_version = parse_version(&upstream_content)
        .ok_or_else(|| anyhow!("cannot parse upstream version.hacker"))?;

        output::label("latest version", &upstream_version);

        // ── 4. Compare ────────────────────────────────────────────────────────────
        if !version_greater(&upstream_version, &local_version) {
            println!();
            output::ok(&format!("HNM is up to date  (v{})", local_version));
            return Ok(());
        }

        println!();
        output::info(&format!(
            "New version available: {} → {}",
            local_version, upstream_version
        ));
        println!();

        // ── 5. Download new binary ────────────────────────────────────────────────
        let download_url = format!("{}/v{}/hnm", UPSTREAM_RELEASE_URL, upstream_version);
        let tmp_bin = std::env::temp_dir().join(".hnm_upgrade_bin");

        {
            let task = progress::TaskProgress::new(100, &format!("downloading hnm v{}", upstream_version));
            task.log(&format!("curl -fsSL {} -o {}", download_url, tmp_bin.display()));

            let result = curl_download(&download_url, &tmp_bin, &task);
            task.inc(70);

            match result {
                Ok(_) => { task.inc(30); task.finish_ok("binary downloaded"); }
                Err(e) => {
                    task.finish_err(&format!("{}", e));
                    return Err(anyhow!("failed to download new binary: {}", e));
                }
            }
        }

        // ── 6. chmod +x ──────────────────────────────────────────────────────────
        let chmod_ok = Command::new("chmod")
        .args(["a+x", tmp_bin.to_str().unwrap()])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

        if !chmod_ok {
            return Err(anyhow!("chmod a+x failed on downloaded binary"));
        }

        // ── 7. sudo mv to /usr/bin/hnm ───────────────────────────────────────────
        output::info("Installing to /usr/bin/hnm  (requires sudo)...");

        {
            let task = progress::TaskProgress::new(100, "installing to /usr/bin/hnm");
            task.log("sudo rm -rf /usr/bin/hnm");

            let rm_ok = Command::new("sudo")
            .args(["rm", "-rf", "/usr/bin/hnm"])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);

            if !rm_ok {
                task.finish_err("sudo rm /usr/bin/hnm failed");
                return Err(anyhow!("failed to remove old /usr/bin/hnm — do you have sudo?"));
            }
            task.inc(30);

            task.log(&format!("sudo mv {} /usr/bin/hnm", tmp_bin.display()));
            let mv_ok = Command::new("sudo")
            .args(["mv", tmp_bin.to_str().unwrap(), "/usr/bin/hnm"])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);

            if !mv_ok {
                task.finish_err("sudo mv failed");
                return Err(anyhow!("failed to move binary to /usr/bin/hnm"));
            }
            task.inc(70);
            task.finish_ok(&format!("hnm v{} installed to /usr/bin/hnm", upstream_version));
        }

        // ── 8. Update local version file ─────────────────────────────────────────
        let new_content = format!("[\n  {}\n]\n", upstream_version);
        fs::write(&local_ver_path, new_content)?;

        println!();
        output::ok(&format!("HNM upgraded  v{}  →  v{}", local_version, upstream_version));
        output::dim("Run `hnm version` to confirm.");

        Ok(())
    }
