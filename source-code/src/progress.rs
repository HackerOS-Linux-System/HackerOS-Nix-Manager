/// HNM progress display
///
/// Layout:
///
///   │ log line 1 ...
///   │ log line 2 ...
///   │ log line 3 ...
///   / [████████████────────────] 55%  resolving dependencies
///
/// Spinner chars: / - \ |
/// Bar style:     ████░░░░ in bright_cyan

use indicatif::{ProgressBar, ProgressStyle, MultiProgress};
use owo_colors::OwoColorize;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const SPINNER_CHARS: [&str; 4] = ["/", "-", "\\", "|"];

/// Build a progress bar with the HNM style.
/// `total` = number of steps (use 0 for indeterminate).
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

/// A scrolling log panel + one progress bar underneath.
/// Call `.log(msg)` to append log lines, `.set_msg(msg)` to update the action label,
/// `.inc(n)` to advance the bar, `.finish_ok(msg)` / `.finish_err(msg)` to complete.
pub struct TaskProgress {
    bar: ProgressBar,
    logs: Arc<Mutex<Vec<String>>>,
}

impl TaskProgress {
    pub fn new(total: u64, label: &str) -> Self {
        let bar = make_bar(total, label);
        Self {
            bar,
            logs: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Print a log line above the bar.
    pub fn log(&self, msg: &str) {
        // Print above the bar using println! — indicatif will redraw bar below
        self.bar.println(format!("  {} {}", "│".cyan().dimmed(), msg.dimmed()));
        if let Ok(mut v) = self.logs.lock() {
            v.push(msg.to_string());
        }
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

    pub fn set_pos(&self, pos: u64) {
        self.bar.set_position(pos);
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

    pub fn abandon(&self) {
        self.bar.abandon();
    }
}

/// Run a subprocess and stream its stdout/stderr as log lines into a TaskProgress.
/// Returns Ok(exit_success) or Err.
pub fn run_with_log(
    task: &TaskProgress,
    cmd: &str,
    args: &[&str],
) -> anyhow::Result<bool> {
    use std::io::{BufRead, BufReader};
    use std::process::{Command, Stdio};

    let mut child = Command::new(cmd)
    .args(args)
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .map_err(|e| anyhow::anyhow!("failed to spawn '{}': {}", cmd, e))?;

    // Stream stdout
    if let Some(stdout) = child.stdout.take() {
        for line in BufReader::new(stdout).lines() {
            if let Ok(l) = line {
                task.log(&l);
            }
        }
    }
    // Stream stderr
    if let Some(stderr) = child.stderr.take() {
        for line in BufReader::new(stderr).lines() {
            if let Ok(l) = line {
                task.err_line(&l);
            }
        }
    }

    let status = child.wait()?;
    Ok(status.success())
}
