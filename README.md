# Tij

**T**ext-mode **I**nterface for **J**ujutsu - A TUI for the [Jujutsu](https://github.com/jj-vcs/jj) version control system, inspired by [tig](https://github.com/jonas/tig).

![Rust](https://img.shields.io/badge/rust-1.85+-orange.svg)
[![Crates.io](https://img.shields.io/crates/v/tij.svg)](https://crates.io/crates/tij)

## Why Tij?

Jujutsu (jj) makes Git's painful operations easy and safe. Tij brings that power to a visual interface:

| Git's Pain | jj's Solution | Tij's UI |
|------------|---------------|----------|
| `git stash` management | Always-committed working copy | One-key context switching |
| `git rebase -i` complexity | `jj edit` + auto-rebase | Visual history editing |
| `git reflog` recovery | `jj undo` / `jj op log` | Operation history view |
| Commit splitting | `jj split` | Integrated diff editor |
| Conflicts block work | Keep conflicts, continue working | Visual conflict status |

## Installation

```bash
# Homebrew (macOS/Linux)
brew tap nakamura-shuta/tij && brew install tij

# From crates.io
cargo install tij

# From source
git clone https://github.com/nakamura-shuta/tij.git
cd tij && cargo install --path .
```

**Requirements**: [Jujutsu](https://github.com/jj-vcs/jj) in PATH (Homebrew installs it automatically)

## Quick Start

```bash
cd /path/to/jj-repo
tij
```

Press `?` for help, `q` to quit.

## Features

| Area | Features |
|------|----------|
| Views | Log (with split-pane preview) / Diff / Status / Help (with `/` search) / Operation History / Blame (with Log jump) / Bookmark / Evolog (evolution history) |
| History Editing | Describe (`d` quick edit / `Ctrl+E` external editor) / Edit / New / New from selected / Commit / Squash / Abandon / Split / Diffedit / Rebase (revision/source/branch/insert-after/insert-before, with `--skip-emptied` toggle) / Absorb / Duplicate / Revert / Simplify Parents |
| Conflict Resolution | Resolve List View / :ours / :theirs / External merge tool / Conflict jump |
| Recovery | Undo / Redo / Operation Restore / Restore file / Restore all |
| Bookmarks | Create / Move to @ (with backward detection) / Delete (multi-select) / Rename / Forget / Track / Untrack / Jump / Bookmark View (`M`) |
| Git Integration | Fetch (multi-remote selection, branch-specific fetch) / Push (with dry-run preview, force push warnings, protected bookmark detection, multi-remote selection, push-by-change, push-by-revision, bulk options: --all/--tracked/--deleted, auto-retry for private commits and empty descriptions) |
| Navigation | Next/Prev (`]`/`[` to move @ through history) / Reversed log order (`V`) |
| Diff | Compare two revisions (`jj diff --from --to`) / Copy to clipboard (`y` full / `Y` diff-only) / Export to `.patch` file (`w`, git unified format) |
| Usability | Revset filtering / Text search / Adaptive status bar / Dynamic context-aware hints / `--limit 200` default (unlimited with revset) |

## Revset Examples

Press `r` to filter commits:

```
all()                    # Show all commits
@-..@                    # Recent commits
author(email)            # By author
ancestors(main)          # Branch history
```

See [jj revset docs](https://jj-vcs.dev/latest/revsets/) for more.

## Default Display

Tij uses jj's default revset (recent/relevant commits). To see all:

1. Press `r`, type `all()`, Enter
2. Or set in `~/.jjconfig.toml`:
   ```toml
   [revsets]
   log = "all()"
   ```

## Development

```bash
cargo test                    # All tests (unit + integration)
cargo test --lib              # Unit tests only
cargo test --tests            # Integration tests only
cargo insta test              # Snapshot tests
```

**Test categories**: Unit (589), Integration (86), Snapshot (17), Property-based (15)

## Acknowledgments

- [Jujutsu](https://github.com/jj-vcs/jj) - The modern VCS
- [tig](https://github.com/jonas/tig) - Inspiration
- [ratatui](https://ratatui.rs/) - TUI framework

## License

MIT
