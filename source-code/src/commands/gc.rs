use anyhow::Result;
use crate::{nix, output, progress};

pub fn run() -> Result<()> {
    nix::ensure_nix()?;
    output::header("Nix garbage collection");

    let before = nix::store_du();
    output::label("store size before", &before);
    println!();

    let task = progress::TaskProgress::new(100, "running nix-store --gc");
    task.log("nix-store --gc");
    nix::gc(&task)?;
    task.finish_ok("garbage collection complete");

    println!();
    let after = nix::store_du();
    output::label("store size after", &after);
    output::ok("done");
    Ok(())
}
