//! Clipboard utilities for diff export
//!
//! Detects available clipboard tools and copies text to the system clipboard.
//! Detection order: pbcopy (macOS) → wl-copy (Wayland) → xclip (X11) → xsel (X11 fallback)

use std::io::Write;
use std::process::{Command, Stdio};

/// Copy text to system clipboard.
///
/// Tries platform-specific commands in priority order:
/// 1. `pbcopy` (macOS)
/// 2. `wl-copy` (Wayland Linux)
/// 3. `xclip -selection clipboard` (X11 Linux)
/// 4. `xsel --clipboard --input` (X11 Linux fallback)
pub fn copy_to_clipboard(text: &str) -> Result<(), String> {
    let commands: &[&[&str]] = &[
        &["pbcopy"],
        &["wl-copy"],
        &["xclip", "-selection", "clipboard"],
        &["xsel", "--clipboard", "--input"],
    ];

    for cmd_args in commands {
        let program = cmd_args[0];
        if !is_available(program) {
            continue;
        }

        let mut child = Command::new(program)
            .args(&cmd_args[1..])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Failed to start {}: {}", program, e))?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(text.as_bytes())
                .map_err(|e| format!("Failed to write to {}: {}", program, e))?;
        }

        let status = child
            .wait()
            .map_err(|e| format!("Failed to wait for {}: {}", program, e))?;

        if status.success() {
            return Ok(());
        }
    }

    Err("No clipboard tool found (install pbcopy, xclip, or wl-copy)".to_string())
}

/// Check if a command is available on the system
fn is_available(program: &str) -> bool {
    Command::new("which")
        .arg(program)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}
