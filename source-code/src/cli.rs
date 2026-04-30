use anyhow::{anyhow, Result};
use lexopt::prelude::*;

pub struct Opts {
    pub command: Command,
}

pub enum Command {
    Search   { query: String, json: bool },
    Install  { packages: Vec<String>, no_env: bool },
    Remove   { packages: Vec<String>, force: bool },
    Update   { packages: Option<Vec<String>> },
    Upgrade,
    Unpack,
    Check,
    Info     { package: String },
    List     { installed: bool, json: bool },
    Env      { sub: String },
    Doctor,
    Clean,
    Rollback { generation: Option<u32> },
    Pin      { package: String, version: Option<String> },
    Unpin    { package: String },
    Which    { package: String },
    Gc,
    Version,
    Help,
}

pub fn parse() -> Result<Opts> {
    let mut parser = lexopt::Parser::from_env();

    // first positional = subcommand
    let subcmd = match parser.next()? {
        Some(Value(v)) => v.string()?,
        Some(Long("help") | Short('h')) => return Ok(Opts { command: Command::Help }),
        Some(Long("version") | Short('V')) => return Ok(Opts { command: Command::Version }),
        Some(arg) => return Err(anyhow!("unexpected argument: {:?}", arg)),
        None => {
            print_help();
            std::process::exit(0);
        }
    };

    let command = match subcmd.as_str() {
        "search" => {
            let mut query = String::new();
            let mut json = false;
            while let Some(arg) = parser.next()? {
                match arg {
                    Long("json") => json = true,
                    Value(v) if query.is_empty() => query = v.string()?,
                    _ => return Err(anyhow!(arg.unexpected())),
                }
            }
            if query.is_empty() {
                return Err(anyhow!("usage: hnm search <query> [--json]"));
            }
            Command::Search { query, json }
        }

        "install" | "i" => {
            let mut packages = Vec::new();
            let mut no_env = false;
            while let Some(arg) = parser.next()? {
                match arg {
                    Long("no-env") => no_env = true,
                    Value(v) => packages.push(v.string()?),
                    _ => return Err(anyhow!(arg.unexpected())),
                }
            }
            if packages.is_empty() {
                return Err(anyhow!("usage: hnm install <package> [<package>...]"));
            }
            Command::Install { packages, no_env }
        }

        "remove" | "rm" | "uninstall" => {
            let mut packages = Vec::new();
            let mut force = false;
            while let Some(arg) = parser.next()? {
                match arg {
                    Long("force") | Short('f') => force = true,
                    Value(v) => packages.push(v.string()?),
                    _ => return Err(anyhow!(arg.unexpected())),
                }
            }
            if packages.is_empty() {
                return Err(anyhow!("usage: hnm remove <package> [<package>...]"));
            }
            Command::Remove { packages, force }
        }

        "update" | "up" => {
            let mut packages: Vec<String> = Vec::new();
            while let Some(arg) = parser.next()? {
                match arg {
                    Value(v) => packages.push(v.string()?),
                    _ => return Err(anyhow!(arg.unexpected())),
                }
            }
            let packages = if packages.is_empty() { None } else { Some(packages) };
            Command::Update { packages }
        }

        "upgrade" => Command::Upgrade,

        "unpack" => Command::Unpack,

        "check" => Command::Check,

        "info" => {
            let package = match parser.next()? {
                Some(Value(v)) => v.string()?,
                _ => return Err(anyhow!("usage: hnm info <package>")),
            };
            Command::Info { package }
        }

        "list" | "ls" => {
            let mut installed = false;
            let mut json = false;
            while let Some(arg) = parser.next()? {
                match arg {
                    Long("installed") | Short('i') => installed = true,
                    Long("json") => json = true,
                    _ => return Err(anyhow!(arg.unexpected())),
                }
            }
            Command::List { installed, json }
        }

        "env" => {
            let sub = match parser.next()? {
                Some(Value(v)) => v.string()?,
                _ => return Err(anyhow!("usage: hnm env <activate|deactivate|status>")),
            };
            Command::Env { sub }
        }

        "doctor" => Command::Doctor,

        "clean" => Command::Clean,

        "rollback" => {
            let generation = match parser.next()? {
                Some(Value(v)) => Some(v.string()?.parse::<u32>()
                    .map_err(|_| anyhow!("generation must be a number"))?),
                None => None,
                Some(arg) => return Err(anyhow!(arg.unexpected())),
            };
            Command::Rollback { generation }
        }

        "pin" => {
            let package = match parser.next()? {
                Some(Value(v)) => v.string()?,
                _ => return Err(anyhow!("usage: hnm pin <package> [<version>]")),
            };
            let version = match parser.next()? {
                Some(Value(v)) => Some(v.string()?),
                None => None,
                Some(arg) => return Err(anyhow!(arg.unexpected())),
            };
            Command::Pin { package, version }
        }

        "unpin" => {
            let package = match parser.next()? {
                Some(Value(v)) => v.string()?,
                _ => return Err(anyhow!("usage: hnm unpin <package>")),
            };
            Command::Unpin { package }
        }

        "which" => {
            let package = match parser.next()? {
                Some(Value(v)) => v.string()?,
                _ => return Err(anyhow!("usage: hnm which <package>")),
            };
            Command::Which { package }
        }

        "gc" => Command::Gc,

        "version" | "--version" | "-V" => Command::Version,

        "help" | "--help" | "-h" => Command::Help,

        other => return Err(anyhow!("unknown command '{}'. Run `hnm help` for usage.", other)),
    };

    Ok(Opts { command })
}

pub fn print_help() {
    use owo_colors::OwoColorize;

    println!();
    println!("  {}  {}", "hnm".bright_cyan().bold(), "HackerOS Nix Manager".cyan());
    println!("  {}", format!("v{}", env!("CARGO_PKG_VERSION")).cyan().dimmed());
    println!();
    println!("{}", "USAGE".bright_cyan().bold().underline());
    println!("  {} {} {}", "hnm".bright_cyan(), "<command>".cyan(), "[options]".cyan().dimmed());
    println!();
    println!("{}", "PACKAGE COMMANDS".bright_cyan().bold().underline());

    let cmds = [
        ("search <query>",       "Search nixpkgs for packages"),
        ("install <pkg...>",     "Install one or more packages"),
        ("remove  <pkg...>",     "Remove one or more packages"),
        ("update  [pkg...]",     "Refresh channel and upgrade packages"),
        ("upgrade",              "Upgrade HNM itself  [placeholder]"),
        ("info    <pkg>",        "Show package details"),
        ("list    [-i]",         "List all / installed packages"),
        ("which   <pkg>",        "Show binary path of a package"),
        ("pin     <pkg> [ver]",  "Pin package to a version"),
        ("unpin   <pkg>",        "Unpin a package"),
        ("rollback [gen]",       "Roll back to a previous generation"),
        ("gc",                   "Run Nix garbage collection"),
    ];
    for (cmd, desc) in &cmds {
        println!("  {}  {}", format!("{:<26}", cmd).bright_cyan(), desc.to_string().dimmed());
    }

    println!();
    println!("{}", "SYSTEM COMMANDS".bright_cyan().bold().underline());
    let sys = [
        ("unpack",   "Bootstrap Nix (install + configure channel)"),
        ("check",    "Verify Nix installation (nix + nix-env versions)"),
        ("doctor",   "Full system diagnostics"),
        ("env <sub>","Manage shell profile  [activate|deactivate|status]"),
        ("clean",    "Remove cached downloads and temp files"),
        ("version",  "Show HNM version info"),
        ("help",     "Show this help"),
    ];
    for (cmd, desc) in &sys {
        println!("  {}  {}", format!("{:<26}", cmd).bright_cyan(), desc.to_string().dimmed());
    }

    println!();
    println!("{}", "FLAGS".bright_cyan().bold().underline());
    println!("  {}  {}", format!("{:<26}", "--json").bright_cyan(), "Output as JSON (search, list)".dimmed());
    println!("  {}  {}", format!("{:<26}", "--no-env").bright_cyan(), "Skip profile activation after install".dimmed());
    println!("  {}  {}", format!("{:<26}", "-f / --force").bright_cyan(), "Force removal".dimmed());
    println!();
    println!("  {} {}","Docs:".cyan(), "https://hackeros-linux-system.github.io/HackerOS-Website/tools-docs/hnm.html".cyan().dimmed());
    println!();
}
