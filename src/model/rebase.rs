/// Rebase operation mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RebaseMode {
    /// `-r`: Move single revision (descendants rebased onto parent)
    #[default]
    Revision,
    /// `-s`: Move revision and all descendants together
    Source,
    /// `-b`: Move entire branch (relative to destination's ancestors)
    Branch,
    /// `-A`: Insert revision after target in history
    InsertAfter,
    /// `-B`: Insert revision before target in history
    InsertBefore,
}
