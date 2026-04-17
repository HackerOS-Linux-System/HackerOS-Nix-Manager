mod cli;
mod commands;
mod config;
mod nix;
mod output;
mod progress;
mod state;

use cli::{Command, Opts};

fn main() {
    let opts = match cli::parse() {
        Ok(o) => o,
        Err(e) => {
            output::error(&e.to_string());
            std::process::exit(1);
        }
    };

    let result = match opts.command {
        Command::Search { query, json }           => commands::search::run(&query, json),
        Command::Install { packages, no_env }     => commands::install::run(&packages, no_env),
        Command::Remove  { packages, force }      => commands::remove::run(&packages, force),
        Command::Update  { packages }             => commands::update::run(packages.as_deref()),
        Command::Upgrade                          => commands::upgrade::run(),
        Command::Unpack                           => commands::unpack::run(),
        Command::Check                            => commands::check::run(),
        Command::Info    { package }              => commands::info::run(&package),
        Command::List    { installed, json }      => commands::list::run(installed, json),
        Command::Env     { sub }                  => commands::env::run(&sub),
        Command::Doctor                           => commands::doctor::run(),
        Command::Clean                            => commands::clean::run(),
        Command::Rollback { generation }          => commands::rollback::run(generation),
        Command::Pin     { package, version }     => commands::pin::run(&package, version.as_deref()),
        Command::Unpin   { package }              => commands::unpin::run(&package),
        Command::Which   { package }              => commands::which::run(&package),
        Command::Gc                               => commands::gc::run(),
        Command::Version                          => { output::version(); Ok(()) },
        Command::Help                             => { cli::print_help(); Ok(()) },
    };

    if let Err(e) = result {
        output::error(&e.to_string());
        std::process::exit(1);
    }
}
