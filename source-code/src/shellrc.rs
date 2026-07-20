use crate::{config, output};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub const MARKER: &str = "# HNM — HackerOS Nix Manager";

const RC_FILES: [&str; 3] = [".bashrc", ".zshrc", ".profile"];

/// Locate the Nix profile activation script (nix.sh), if any.
pub fn find_nix_sh() -> Option<PathBuf> {
    let home = config::home();
    [
        home.join(".nix-profile/etc/profile.d/nix.sh"),
        PathBuf::from("/nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh"),
        PathBuf::from("/etc/profile.d/nix.sh"),
        home.join(".local/state/nix/profiles/profile/etc/profile.d/nix.sh"),
        PathBuf::from("/root/.nix-profile/etc/profile.d/nix.sh"),
    ]
    .into_iter()
    .find(|p| p.exists())
}

fn block_for(profile_bin: &Path) -> String {
    let nix_sh = find_nix_sh();
    let nix_sh_line = match nix_sh {
        Some(ref p) => format!("[ -f '{}' ] && . '{}'", p.display(), p.display()),
        None => "[ -f ~/.nix-profile/etc/profile.d/nix.sh ] && \\\n    . ~/.nix-profile/etc/profile.d/nix.sh"
            .to_string(),
    };
    format!(
        "\n{marker}\nexport PATH=\"{bin}:$HOME/.nix-profile/bin:$PATH\"\n{nix_sh_line}\n",
        marker = MARKER,
        bin = profile_bin.display(),
    )
}

/// Append the HNM PATH block to every rc file that exists and isn't
/// already patched. Returns the list of files that were newly patched.
pub fn patch(profile_bin: &Path) -> Vec<String> {
    let home = config::home();
    let block = block_for(profile_bin);
    let mut patched = Vec::new();

    for rc in RC_FILES {
        let rc_path = home.join(rc);
        if !rc_path.exists() {
            continue;
        }
        let content = fs::read_to_string(&rc_path).unwrap_or_default();
        if content.contains(MARKER) {
            output::dim(&format!("  ~/{} — already patched", rc));
            continue;
        }
        match fs::OpenOptions::new().append(true).open(&rc_path) {
            Ok(mut f) => {
                if f.write_all(block.as_bytes()).is_ok() {
                    output::ok(&format!("patched ~/{}", rc));
                    patched.push(rc.to_string());
                } else {
                    output::warn(&format!("could not write to ~/{}", rc));
                }
            }
            Err(e) => output::warn(&format!("could not patch ~/{}: {}", rc, e)),
        }
    }

    patched
}

/// Remove the HNM block (everything from the marker line up to and
/// including the following two lines) from every rc file that has it.
/// Returns the list of files that were unpatched.
pub fn unpatch() -> Vec<String> {
    let home = config::home();
    let mut unpatched = Vec::new();

    for rc in RC_FILES {
        let rc_path = home.join(rc);
        if !rc_path.exists() {
            continue;
        }
        let content = match fs::read_to_string(&rc_path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        if !content.contains(MARKER) {
            continue;
        }

        // Drop the marker line plus the `export PATH=...` and nix.sh
        // sourcing lines that immediately follow it. Anything else the
        // user added around the block is left untouched.
        let lines: Vec<&str> = content.lines().collect();
        let mut out_lines: Vec<&str> = Vec::with_capacity(lines.len());
        let mut skip_remaining = 0usize;
        for line in lines {
            if skip_remaining > 0 {
                skip_remaining -= 1;
                continue;
            }
            if line.trim() == MARKER {
                skip_remaining = 2; // export PATH line + nix.sh line
                continue;
            }
            out_lines.push(line);
        }

        // Collapse any run of blank lines left behind by the removal.
        let mut cleaned = String::new();
        let mut prev_blank = false;
        for line in out_lines {
            let is_blank = line.trim().is_empty();
            if is_blank && prev_blank {
                continue;
            }
            cleaned.push_str(line);
            cleaned.push('\n');
            prev_blank = is_blank;
        }

        match fs::write(&rc_path, cleaned) {
            Ok(_) => {
                output::ok(&format!("unpatched ~/{}", rc));
                unpatched.push(rc.to_string());
            }
            Err(e) => output::warn(&format!("could not unpatch ~/{}: {}", rc, e)),
        }
    }

    unpatched
}
