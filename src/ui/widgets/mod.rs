//! Reusable UI widgets

mod error_banner;
mod help_panel;
mod placeholder;
mod status_bar;

pub use error_banner::{MIN_VIEW_ROWS, error_banner_height, render_error_banner};
pub use help_panel::{matching_line_indices, render_help_panel};
pub use placeholder::render_placeholder;
pub use status_bar::{
    blame_status_height, diff_status_height, render_blame_status_bar, render_diff_status_bar,
    render_status_hints, status_hints_height,
};
