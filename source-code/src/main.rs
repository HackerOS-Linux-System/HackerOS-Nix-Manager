mod cli;
mod commands;
mod config;
mod nix;
mod output;
mod state;

use clap::Parser;
use cli::{Cli, Commands};

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Search { query, json } => commands::search::run(&query, json),
        Commands::Install { packages, no_env } => commands::install::run(&packages, no_env),
        Commands::Remove { packages, force } => commands::remove::run(&packages, force),
        Commands::Update { packages } => commands::update::run(packages.as_deref()),
        Commands::Upgrade => commands::upgrade::run(),
        Commands::Info { package } => commands::info::run(&package),
        Commands::List { installed, json } => commands::list::run(installed, json),
        Commands::Env { subcommand } => commands::env::run(subcommand),
        Commands::Doctor => commands::doctor::run(),
        Commands::Clean => commands::clean::run(),
        Commands::Rollback { generation } => commands::rollback::run(generation),
        Commands::Pin { package, version } => commands::pin::run(&package, version.as_deref()),
        Commands::Unpin { package } => commands::unpin::run(&package),
        Commands::Which { package } => commands::which::run(&package),
        Commands::Gc => commands::gc::run(),
        Commands::Version => {
            output::print_version();
            Ok(())
        }
    };

    if let Err(e) = result {
        output::print_error(&e.to_string());
        std::process::exit(1);
    }
}
