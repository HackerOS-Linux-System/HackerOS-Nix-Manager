use indicatif::{ProgressBar, ProgressStyle};
use owo_colors::OwoColorize;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const SPINNER_CHARS: [&str; 4] = ["/", "-", "\\", "|"];

pub fn make_bar(total: u64, label: &str) -> ProgressBar {
    let bar = if total == 0 {
        ProgressBar::new_spinner()
    } else {
        ProgressBar::new(total)
    };
    let style = ProgressStyle::with_template(
        "  {spinner:.cyan}  {bar:40.cyan/black}  {percent:>3}%  {msg:.bright_cyan}",
    )
    .unwrap()
    .tick_strings(&SPINNER_CHARS)
    .progress_chars("█▉▊▋▌▍▎▏ ");
    bar.set_style(style);
    bar.set_message(label.to_string());
    bar.enable_steady_tick(Duration::from_millis(120));
    bar
}

pub struct TaskProgress {
    bar: ProgressBar,
    _logs: Arc<Mutex<Vec<String>>>,
}

impl TaskProgress {
    pub fn new(total: u64, label: &str) -> Self {
        Self {
            bar: make_bar(total, label),
            _logs: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn log(&self, msg: &str) {
        self.bar.println(format!("  {} {}", "│".cyan().dimmed(), msg.dimmed()));
    }

    pub fn warn(&self, msg: &str) {
        self.bar.println(format!(
            "  {} {}",
            "│ WARN".yellow().underline(),
            msg.yellow().underline()
        ));
    }

    pub fn err_line(&self, msg: &str) {
        self.bar.println(format!(
            "  {} {}",
            "│ ERR ".red().bold().underline(),
            msg.red().bold().underline()
        ));
    }

    pub fn set_msg(&self, msg: &str) {
        self.bar.set_message(msg.to_string());
    }

    pub fn inc(&self, delta: u64) {
        self.bar.inc(delta);
    }

    pub fn finish_ok(&self, msg: &str) {
        self.bar.set_style(
            ProgressStyle::with_template(
                "  {spinner:.cyan}  {bar:40.cyan/black}  100%  {msg:.bright_cyan}",
            )
            .unwrap()
            .tick_strings(&SPINNER_CHARS)
            .progress_chars("█▉▊▋▌▍▎▏ "),
        );
        self.bar.finish_with_message(format!("✓ {}", msg));
    }

    pub fn finish_err(&self, msg: &str) {
        self.bar.set_style(
            ProgressStyle::with_template(
                "  {spinner:.red}  {bar:40.red/black}   ERR  {msg:.red}",
            )
            .unwrap()
            .tick_strings(&SPINNER_CHARS)
            .progress_chars("█▉▊▋▌▍▎▏ "),
        );
        self.bar.abandon_with_message(format!("✗ {}", msg));
    }
}

/// Run a pre-built Command, streaming stdout+stderr as log lines.
/// Returns Ok(true) if exit code 0, Ok(false) otherwise.
pub fn run_cmd_log(
    task: &TaskProgress,
    mut cmd: std::process::Command,
) -> anyhow::Result<bool> {
    use std::io::{BufRead, BufReader};

    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| anyhow::anyhow!("failed to spawn process: {}", e))?;

    // Stream stdout
    if let Some(stdout) = child.stdout.take() {
        for line in BufReader::new(stdout).lines() {
            if let Ok(l) = line {
                task.log(&l);
            }
        }
    }
    // Stream stderr — distinguish errors from info
    if let Some(stderr) = child.stderr.take() {
        for line in BufReader::new(stderr).lines() {
            if let Ok(l) = line {
                let lc = l.to_lowercase();
                if lc.contains("error:") || lc.starts_with("error") {
                    task.err_line(&l);
                } else {
                    task.log(&l);
                }
            }
        }
    }

    let status = child.wait()?;
    Ok(status.success())
}

/// Convenience: run a shell string with env vars.
pub fn run_shell_log_env(
    task: &TaskProgress,
    shell_cmd: &str,
    env_vars: &[(String, String)],
) -> anyhow::Result<bool> {
    let mut cmd = std::process::Command::new("sh");
    cmd.args(["-c", shell_cmd])
        .envs(env_vars.iter().map(|(k, v)| (k.as_str(), v.as_str())));
    run_cmd_log(task, cmd)
}

/// Compatibility shim used by unpack.rs and others.
pub fn run_with_log_env(
    task: &TaskProgress,
    program: &str,
    args: &[&str],
    env_vars: &[(String, String)],
) -> anyhow::Result<bool> {
    let mut cmd = std::process::Command::new(program);
    cmd.args(args)
        .envs(env_vars.iter().map(|(k, v)| (k.as_str(), v.as_str())));
    run_cmd_log(task, cmd)
}

pub fn run_with_log(
    task: &TaskProgress,
    program: &str,
    args: &[&str],
) -> anyhow::Result<bool> {
    run_with_log_env(task, program, args, &[])
}
