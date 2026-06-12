//! Display-width-aware text fitting for fixed-width list columns.
//!
//! `format!("{:<N}", …)` pads by *char count*, so CJK content (2-cell chars)
//! overflows its column and shifts everything after it. This module fits
//! strings by **terminal cell width** via ratatui's [`CellWidth`], which also
//! handles the halfwidth-katakana dakuten quirks the terminal does.

use ratatui::buffer::CellWidth;

/// Display width of `s` in terminal cells.
pub fn display_width(s: &str) -> usize {
    s.cell_width() as usize
}

/// Fit `s` into exactly `width` terminal cells: truncate (with `...`) when too
/// wide, pad with spaces when narrower. The result always occupies `width`
/// cells, so fixed columns stay aligned even with CJK content.
pub fn fit_display_width(s: &str, width: usize) -> String {
    let mut out = String::new();
    let mut used = 0usize;

    if display_width(s) <= width {
        out.push_str(s);
        used = display_width(s);
    } else {
        // Reserve room for the ellipsis when there is any (matches the old
        // char-based truncate_str behavior: no ellipsis for width <= 3).
        let ellipsis = if width > 3 { 3 } else { 0 };
        let budget = width - ellipsis;
        let mut buf = [0u8; 4];
        for ch in s.chars() {
            let w = display_width(ch.encode_utf8(&mut buf));
            if used + w > budget {
                break;
            }
            out.push(ch);
            used += w;
        }
        for _ in 0..ellipsis {
            out.push('.');
        }
        used += ellipsis;
    }

    for _ in used..width {
        out.push(' ');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_pads_to_width() {
        assert_eq!(fit_display_width("main", 8), "main    ");
        assert_eq!(fit_display_width("exactly8", 8), "exactly8");
    }

    #[test]
    fn ascii_truncates_with_ellipsis() {
        assert_eq!(fit_display_width("feature/login-flow", 10), "feature...");
    }

    #[test]
    fn cjk_counts_two_cells_per_char() {
        // 5 chars × 2 cells = 10 cells — fits exactly in 10
        assert_eq!(fit_display_width("あいうえお", 10), "あいうえお");
        // width 8: 2 chars (4 cells) + "..." (3) = 7 cells, pad 1
        assert_eq!(fit_display_width("あいうえお", 8), "あい... ");
    }

    #[test]
    fn cjk_pads_by_cells_not_chars() {
        // "あい" = 4 cells; padding must bring it to 8 CELLS (4 spaces),
        // where char-based {:<8} would have added 6 and overflowed.
        assert_eq!(fit_display_width("あい", 8), "あい    ");
    }

    #[test]
    fn mixed_ascii_cjk() {
        // "fix-バグ" = 4 + 4 = 8 cells
        assert_eq!(fit_display_width("fix-バグ", 8), "fix-バグ");
        assert_eq!(fit_display_width("fix-バグ修正", 8), "fix-... ");
    }

    #[test]
    fn tiny_width_no_ellipsis() {
        assert_eq!(fit_display_width("abcdef", 3), "abc");
        assert_eq!(fit_display_width("あいう", 3), "あ "); // 2 cells + 1 pad
    }

    #[test]
    fn empty_and_zero() {
        assert_eq!(fit_display_width("", 4), "    ");
        assert_eq!(fit_display_width("abc", 0), "");
    }
}
