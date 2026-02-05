//! Shared navigation helpers for list-based Views
//!
//! Pure functions that compute new selection/scroll state without side effects.
//! Each View calls these with its own state and applies the results.

/// Move selection down by one, clamped to max_index.
///
/// Returns the new selected index.
pub fn select_next(selected: usize, max_index: usize) -> usize {
    selected.saturating_add(1).min(max_index)
}

/// Move selection up by one.
///
/// Returns the new selected index.
pub fn select_prev(selected: usize) -> usize {
    selected.saturating_sub(1)
}

/// Calculate scroll offset to keep `selected` visible within `visible_count` rows.
///
/// If `visible_count` is 0, returns `scroll_offset` unchanged (no-op).
/// Usable at both input time and render time.
pub fn adjust_scroll(selected: usize, scroll_offset: usize, visible_count: usize) -> usize {
    if visible_count == 0 {
        return scroll_offset;
    }
    if selected < scroll_offset {
        selected
    } else if selected >= scroll_offset + visible_count {
        selected - visible_count + 1
    } else {
        scroll_offset
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // select_next tests
    // =========================================================================

    #[test]
    fn test_select_next_basic() {
        assert_eq!(select_next(0, 5), 1);
        assert_eq!(select_next(4, 5), 5);
    }

    #[test]
    fn test_select_next_at_max() {
        assert_eq!(select_next(5, 5), 5);
    }

    #[test]
    fn test_select_next_zero_max() {
        assert_eq!(select_next(0, 0), 0);
    }

    // =========================================================================
    // select_prev tests
    // =========================================================================

    #[test]
    fn test_select_prev_basic() {
        assert_eq!(select_prev(3), 2);
        assert_eq!(select_prev(1), 0);
    }

    #[test]
    fn test_select_prev_at_zero() {
        assert_eq!(select_prev(0), 0);
    }

    // =========================================================================
    // adjust_scroll tests
    // =========================================================================

    #[test]
    fn test_adjust_scroll_no_change_needed() {
        // selected=3, scroll=0, visible=10 → 3 is within [0..10), no change
        assert_eq!(adjust_scroll(3, 0, 10), 0);
    }

    #[test]
    fn test_adjust_scroll_selected_above_viewport() {
        // selected=2, scroll=5, visible=10 → 2 < 5, scroll down to 2
        assert_eq!(adjust_scroll(2, 5, 10), 2);
    }

    #[test]
    fn test_adjust_scroll_selected_below_viewport() {
        // selected=15, scroll=0, visible=10 → 15 >= 0+10, scroll to 15-10+1=6
        assert_eq!(adjust_scroll(15, 0, 10), 6);
    }

    #[test]
    fn test_adjust_scroll_visible_count_zero() {
        // visible_count=0 → no-op, return existing scroll_offset
        assert_eq!(adjust_scroll(5, 3, 0), 3);
    }

    #[test]
    fn test_adjust_scroll_edge_just_visible() {
        // selected=9, scroll=0, visible=10 → 9 < 0+10, still visible
        assert_eq!(adjust_scroll(9, 0, 10), 0);
    }

    #[test]
    fn test_adjust_scroll_edge_just_outside() {
        // selected=10, scroll=0, visible=10 → 10 >= 0+10, need scroll
        assert_eq!(adjust_scroll(10, 0, 10), 1);
    }
}
