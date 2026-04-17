use anyhow::Result;
use crate::output;

pub fn run() -> Result<()> {
    output::header("HNM Self-Upgrade");
    output::warn("Self-upgrade is not yet implemented in this version.");
    output::info("HNM is bundled with HackerOS and updated through system updates.");
    output::dim("Stay tuned for a future release.");
    Ok(())
}
