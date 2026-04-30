use owo_colors::OwoColorize;

// ─── COLOR SCHEME ─────────────────────────────────────────────────────────────
//  errors      → red   bold + underline
//  warnings    → yellow underline
//  primary     → bright_cyan  (jasny niebieski)
//  secondary   → cyan         (ciemny niebieski)
// ──────────────────────────────────────────────────────────────────────────────

pub fn error(msg: &str) {
    eprintln!("{} {}",
        " ERROR ".red().bold().underline(),
        msg.red().bold().underline(),
    );
}

pub fn warn(msg: &str) {
    eprintln!("{} {}",
        " WARN ".yellow().underline(),
        msg.yellow().underline(),
    );
}

pub fn ok(msg: &str) {
    println!("{} {}", "✓".bright_cyan().bold(), msg.bright_cyan());
}

pub fn info(msg: &str) {
    println!("{} {}", "·".cyan(), msg);
}

pub fn dim(msg: &str) {
    println!("  {}", msg.cyan().dimmed());
}

pub fn header(title: &str) {
    let bar = "─".repeat(title.len() + 4);
    println!();
    println!("  {} {}", "▸".bright_cyan().bold(), title.bright_cyan().bold());
    println!("  {}", bar.cyan().dimmed());
}

pub fn label(key: &str, val: &str) {
    println!("  {:>16}  {}", key.cyan(), val.bright_cyan());
}

pub fn table_header() {
    println!(
        "  {:<32}  {:<14}  {}",
        "PACKAGE".cyan().bold().underline().to_string(),
        "VERSION".cyan().bold().underline().to_string(),
        "DESCRIPTION".cyan().bold().underline().to_string(),
    );
    println!("  {}", "─".repeat(80).cyan().dimmed());
}

pub fn table_row(name: &str, version: &str, desc: &str, is_installed: bool) {
    let marker = if is_installed { " ✓" } else { "" };
    let full_name = format!("{}{}", name, marker);
    let short_desc: String = desc.chars().take(52).collect();
    println!(
        "  {:<32}  {:<14}  {}",
        full_name.bright_cyan().bold(),
        version.cyan(),
        short_desc.dimmed(),
    );
}

pub fn log_line(msg: &str) {
    println!("  {} {}", "│".cyan().dimmed(), msg.dimmed());
}

pub fn step(tag: &str, msg: &str) {
    println!("  {} {}", format!("[{}]", tag).cyan().bold(), msg.bright_cyan());
}

pub fn version() {
    println!();
    println!("  {} {}{}", "hnm".bright_cyan().bold(), "v".cyan(), env!("CARGO_PKG_VERSION").bright_cyan().bold());
    println!("  {}", "HackerOS Nix Manager".cyan());
    println!("  {}", "Part of the HackerOS ecosystem".cyan().dimmed());
    println!();

    let nix_ver = std::process::Command::new("nix")
        .arg("--version")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "not installed".into());

    let nix_env_ver = std::process::Command::new("nix-env")
        .arg("--version")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "not installed".into());

    label("nix", &nix_ver);
    label("nix-env", &nix_env_ver);
    label("platform", std::env::consts::OS);
    label("arch", std::env::consts::ARCH);
    println!();
}
