use anyhow::Result;
use std::process::Command;
use crate::output;

pub fn run() -> Result<()> {
    output::header("Nix installation check");

    let nix_ver = Command::new("nix").arg("--version").output();
    match nix_ver {
        Ok(o) if o.status.success() => {
            let v = String::from_utf8_lossy(&o.stdout).trim().to_string();
            output::ok(&format!("nix        {}", v));
        }
        _ => {
            output::warn("nix        NOT FOUND");
            output::dim("Run `hnm unpack` to install Nix.");
        }
    }

    let env_ver = Command::new("nix-env").arg("--version").output();
    match env_ver {
        Ok(o) if o.status.success() => {
            let v = String::from_utf8_lossy(&o.stdout).trim().to_string();
            output::ok(&format!("nix-env    {}", v));
        }
        _ => {
            output::warn("nix-env    NOT FOUND");
        }
    }

    let chan = Command::new("nix-channel").arg("--list").output();
    match chan {
        Ok(o) if o.status.success() => {
            let v = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if v.is_empty() {
                output::warn("channels   (none registered)");
                output::dim("Run `hnm unpack` to add nixpkgs-unstable.");
            } else {
                output::ok(&format!("channels   {}", v.replace('\n', "  |  ")));
            }
        }
        _ => output::warn("nix-channel  NOT FOUND"),
    }

    println!();
    Ok(())
}
