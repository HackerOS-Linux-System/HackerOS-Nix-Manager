use anyhow::Result;
use chrono::Utc;
use crate::{nix, output, progress, state};

pub fn run(packages: Option<&[String]>) -> Result<()> {
    nix::ensure_nix()?;
    output::header("Update HNM");

    // ── Step 1: refresh channel ──────────────────────────────────────────────
    {
        let task = progress::TaskProgress::new(100, "refreshing nixpkgs channel");
        nix::update_channel(&task)?;
        task.finish_ok("channel refreshed");
    }

    println!();

    // ── Step 2: upgrade profile ──────────────────────────────────────────────
    {
        let label = match packages {
            Some(p) => format!("upgrading {} package(s)", p.len()),
            None    => "upgrading all packages".into(),
        };
        let task = progress::TaskProgress::new(100, &label);
        nix::upgrade_profile(&task)?;
        task.finish_ok("packages upgraded");
    }

    let mut st = state::load()?;
    st.last_update = Some(Utc::now());
    state::save(&st)?;

    println!();
    output::ok("HNM is up to date");
    Ok(())
}
