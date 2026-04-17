use anyhow::{anyhow, Result};
use crate::{nix, output, progress};

pub fn run(generation: Option<u32>) -> Result<()> {
    nix::ensure_nix()?;
    output::header("Rollback");

    let gens = nix::list_generations()?;

    if gens.is_empty() {
        return Err(anyhow!("no generations found in profile"));
    }

    output::dim("Available generations:");
    for (num, date, current) in &gens {
        let marker = if *current { "  ← current" } else { "" };
        output::dim(&format!("  gen {:>4}   {}{}", num, date, marker));
    }
    println!();

    let target = match generation {
        Some(g) => g,
        None => {
            // find current gen, go one back
            let current_idx = gens.iter().position(|(_, _, c)| *c);
            match current_idx {
                Some(i) if i > 0 => gens[i - 1].0,
                _ => return Err(anyhow!("cannot determine previous generation")),
            }
        }
    };

    // Confirm target exists
    if !gens.iter().any(|(n, _, _)| *n == target) {
        return Err(anyhow!("generation {} does not exist", target));
    }

    let task = progress::TaskProgress::new(100, &format!("switching to generation {}", target));
    task.log(&format!("nix-env --switch-generation {}", target));
    nix::switch_generation(target, &task)?;
    task.finish_ok(&format!("rolled back to generation {}", target));

    println!();
    output::ok(&format!("now at generation {}", target));
    output::dim("Run `hnm list` to see current packages.");
    Ok(())
}
