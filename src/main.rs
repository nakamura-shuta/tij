//! Tij - Text-mode Interface for Jujutsu
//!
//! Binary entry point for the TUI application.

use std::time::Duration;

use crossterm::event::{self, Event, KeyEventKind};
use ratatui::DefaultTerminal;

use tij::app::App;

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    // jj version check (before TUI init so errors print to normal terminal)
    check_jj_version()?;

    let terminal = ratatui::init();
    let result = run(terminal);
    ratatui::restore();
    result
}

/// Minimum required jj version (major, minor)
const MIN_JJ_VERSION: (u32, u32) = (0, 40);

/// Check that jj is installed and meets the minimum version requirement.
fn check_jj_version() -> color_eyre::Result<()> {
    use color_eyre::eyre::eyre;
    use std::process::Command;

    // 1. Check jj exists
    let output = Command::new("jj")
        .arg("version")
        .output()
        .map_err(|_| eyre!("jj not found. Please install jj: https://github.com/jj-vcs/jj"))?;

    // 2. Check jj version succeeded
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(eyre!(
            "jj version failed (exit code: {}):\n{}",
            output.status.code().unwrap_or(-1),
            stderr.trim()
        ));
    }

    let version_str = String::from_utf8_lossy(&output.stdout);

    // 3. Parse version
    let (major, minor) = parse_jj_version(&version_str)
        .ok_or_else(|| eyre!("Could not parse jj version: {}", version_str.trim()))?;

    // 4. Check minimum version
    if (major, minor) < MIN_JJ_VERSION {
        return Err(eyre!(
            "tij requires jj {}.{}.0 or later (found {}.{}).\n\
             Please upgrade: https://github.com/jj-vcs/jj/releases",
            MIN_JJ_VERSION.0,
            MIN_JJ_VERSION.1,
            major,
            minor
        ));
    }

    Ok(())
}

/// Parse jj version string into (major, minor).
///
/// Handles formats like "jj 0.40.0", "jj 0.40.0-rc1", "jj 1.0.0.dev1234".
fn parse_jj_version(output: &str) -> Option<(u32, u32)> {
    let version_str = output.trim().strip_prefix("jj ")?;
    let parts: Vec<&str> = version_str.split('.').collect();
    if parts.len() >= 2 {
        let major = parse_leading_digits(parts[0])?;
        let minor = parse_leading_digits(parts[1])?;
        Some((major, minor))
    } else {
        None
    }
}

/// Extract leading digits from a string (e.g., "40" -> 40, "40-rc1" -> 40).
fn parse_leading_digits(s: &str) -> Option<u32> {
    let digits: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        None
    } else {
        digits.parse().ok()
    }
}

/// Run the application's main loop.
fn run(mut terminal: DefaultTerminal) -> color_eyre::Result<()> {
    let mut app = App::new();

    while app.running {
        terminal.draw(|frame| app.render(frame))?;
        handle_events(&mut app)?;
    }

    Ok(())
}

/// Handle crossterm events.
///
/// Uses poll with 200ms timeout to support idle processing (e.g., debounced preview fetch).
/// When no key event arrives within the timeout, pending preview fetches are resolved.
fn handle_events(app: &mut App) -> color_eyre::Result<()> {
    if event::poll(Duration::from_millis(200))? {
        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                app.on_key_event(key);
            }
            _ => {}
        }
    } else {
        // Idle: resolve any pending preview fetch
        app.resolve_pending_preview();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_jj_version_normal() {
        assert_eq!(parse_jj_version("jj 0.40.0"), Some((0, 40)));
    }

    #[test]
    fn test_parse_jj_version_older() {
        assert_eq!(parse_jj_version("jj 0.39.1"), Some((0, 39)));
    }

    #[test]
    fn test_parse_jj_version_major() {
        assert_eq!(parse_jj_version("jj 1.0.0"), Some((1, 0)));
    }

    #[test]
    fn test_parse_jj_version_rc_suffix() {
        assert_eq!(parse_jj_version("jj 0.40.0-rc1"), Some((0, 40)));
    }

    #[test]
    fn test_parse_jj_version_dev_suffix() {
        assert_eq!(parse_jj_version("jj 0.40.0.dev1234"), Some((0, 40)));
    }

    #[test]
    fn test_parse_jj_version_with_trailing_newline() {
        assert_eq!(parse_jj_version("jj 0.40.0\n"), Some((0, 40)));
    }

    #[test]
    fn test_parse_jj_version_invalid() {
        assert_eq!(parse_jj_version("invalid"), None);
    }

    #[test]
    fn test_parse_jj_version_empty() {
        assert_eq!(parse_jj_version(""), None);
    }

    #[test]
    fn test_parse_leading_digits_normal() {
        assert_eq!(parse_leading_digits("40"), Some(40));
    }

    #[test]
    fn test_parse_leading_digits_with_suffix() {
        assert_eq!(parse_leading_digits("40-rc1"), Some(40));
    }

    #[test]
    fn test_parse_leading_digits_empty() {
        assert_eq!(parse_leading_digits(""), None);
    }

    #[test]
    fn test_parse_leading_digits_no_digits() {
        assert_eq!(parse_leading_digits("abc"), None);
    }
}
