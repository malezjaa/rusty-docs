use std::{
    io::{self, IsTerminal},
    path::Path,
};

use indicatif::{ProgressBar, ProgressStyle};
use yansi::{Color, Paint};

const ACCENT: Color = Color::Rgb(202, 91, 55);
const MUTED: Color = Color::Rgb(137, 143, 151);
const VALUE: Color = Color::Rgb(121, 190, 216);
const SUCCESS: Color = Color::Rgb(112, 190, 140);

pub struct Terminal {
    interactive: bool,
    color: bool,
    stdout_color: bool,
    unicode: bool,
    quiet: bool,
    verbose: bool,
}

pub struct Stage {
    bar: Option<ProgressBar>,
    plain: bool,
    quiet: bool,
    prefix: String,
    done_marker: String,
}

impl Terminal {
    pub fn new(quiet: bool, verbose: bool) -> Self {
        let interactive = io::stderr().is_terminal() && !quiet;
        let color_enabled = std::env::var_os("NO_COLOR").is_none()
            && std::env::var("TERM").is_ok_and(|term| term != "dumb");
        let color = interactive && color_enabled;
        let stdout_color = io::stdout().is_terminal() && !quiet && color_enabled;
        let locale = ["LC_ALL", "LC_CTYPE", "LANG"]
            .into_iter()
            .find_map(|key| std::env::var(key).ok())
            .unwrap_or_default()
            .to_ascii_lowercase();
        let unicode = locale.contains("utf-8") || locale.contains("utf8");
        Self {
            interactive,
            color,
            stdout_color,
            unicode,
            quiet,
            verbose,
        }
    }

    pub fn stage(&self, message: impl Into<String>) -> Stage {
        let message = message.into();
        let prefix = self.paint("rusty-docs", ACCENT);
        let done_marker = self.done_marker();
        if self.interactive {
            let bar = ProgressBar::new_spinner();
            bar.set_prefix(prefix.clone());
            bar.set_message(message.clone());
            let spinner = if self.unicode {
                &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"][..]
            } else {
                &["-", "\\", "|", "/"][..]
            };
            bar.set_style(
                ProgressStyle::with_template("{prefix:.bold} {spinner:.cyan} {msg}")
                    .expect("valid spinner style")
                    .tick_strings(spinner),
            );
            bar.enable_steady_tick(std::time::Duration::from_millis(90));
            Stage {
                bar: Some(bar),
                plain: false,
                quiet: false,
                prefix,
                done_marker,
            }
        } else {
            if !self.quiet {
                eprintln!("... {message}");
            }
            Stage {
                bar: None,
                plain: true,
                quiet: self.quiet,
                prefix,
                done_marker,
            }
        }
    }

    pub fn progress(&self, total: u64, message: &str) -> Option<ProgressBar> {
        if !self.interactive || total == 0 {
            if !self.quiet && total > 0 {
                eprintln!("... {message} ({total} items)");
            }
            return None;
        }

        let bar = ProgressBar::new(total);
        let prefix = self.paint("rusty-docs", ACCENT);
        bar.set_prefix(prefix);
        bar.set_message(message.to_owned());
        let progress_chars = if self.unicode { "━━╸─" } else { "##- " };
        bar.set_style(
            ProgressStyle::with_template("{prefix:.bold} {wide_bar:.cyan/blue} {pos}/{len} {msg}")
                .expect("valid progress style")
                .progress_chars(progress_chars),
        );
        Some(bar)
    }

    pub fn finish_progress(&self, bar: ProgressBar, message: &str) {
        bar.finish_and_clear();
        eprintln!(
            "{} {} {message}",
            self.paint("rusty-docs", ACCENT),
            self.done_marker()
        );
    }

    pub fn note(&self, message: &str) {
        if self.quiet {
            return;
        }
        eprintln!("{} {message}", self.paint("info", MUTED));
    }

    pub fn is_verbose(&self) -> bool {
        self.verbose
    }

    pub fn is_color_enabled(&self) -> bool {
        self.color
    }

    pub fn highlight(&self, value: &str) -> String {
        self.paint(value, VALUE)
    }

    pub fn format_path(&self, path: &Path) -> String {
        self.format_path_with_color(path, self.stdout_color)
    }

    pub fn format_log_path(&self, path: &Path) -> String {
        self.format_path_with_color(path, self.color)
    }

    fn format_path_with_color(&self, path: &Path, color: bool) -> String {
        let current_dir = std::env::current_dir().ok();
        let absolute_path = if path.is_absolute() {
            path.to_path_buf()
        } else if let Some(current_dir) = current_dir.as_ref() {
            current_dir.join(path)
        } else {
            path.to_path_buf()
        };
        let relative_path = current_dir
            .as_deref()
            .and_then(|current_dir| pathdiff::diff_paths(&absolute_path, current_dir))
            .unwrap_or(absolute_path);
        let display = if relative_path.as_os_str().is_empty() {
            ".".to_owned()
        } else {
            relative_path.display().to_string()
        };
        if color {
            display.dim().to_string()
        } else {
            display
        }
    }

    fn done_marker(&self) -> String {
        if self.unicode {
            self.paint("✓", SUCCESS)
        } else {
            self.paint("ok", SUCCESS)
        }
    }

    fn paint(&self, text: &str, color: Color) -> String {
        if self.color {
            text.paint(color.foreground()).to_string()
        } else {
            text.to_owned()
        }
    }
}

impl Stage {
    pub fn finish(mut self, message: &str) {
        if let Some(bar) = self.bar.take() {
            bar.finish_and_clear();
            eprintln!("{} {} {message}", self.prefix, self.done_marker);
        } else if self.plain && !self.quiet {
            eprintln!("{} {message}", self.done_marker);
        }
    }
}

impl Drop for Stage {
    fn drop(&mut self) {
        if let Some(bar) = self.bar.take() {
            bar.finish_and_clear();
        }
    }
}

pub fn print_error(message: &str) {
    let color = std::io::stderr().is_terminal()
        && std::env::var_os("NO_COLOR").is_none()
        && std::env::var("TERM").is_ok_and(|term| term != "dumb");
    if color {
        eprintln!(
            "{}: {message}",
            "error".paint(Color::Rgb(201, 67, 67).foreground())
        );
    } else {
        eprintln!("error: {message}");
    }
}
