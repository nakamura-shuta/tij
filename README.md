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

### Implemented

| Area | Features |
|------|----------|
| Views | Log / Diff / Status / Help / Operation History / Blame / Bookmark |
| History Editing | Describe (`d` quick edit / `Ctrl+E` external editor) / Edit / New / New from selected / Commit / Squash / Abandon / Split / Rebase (revision/source/insert-after/insert-before) / Absorb |
| Conflict Resolution | Resolve List View / :ours / :theirs / External merge tool / Conflict jump |
| Recovery | Undo / Redo / Operation Restore |
| Bookmarks | Create / Move / Delete (multi-select) / Track / Untrack / Jump / Bookmark View (`M`) |
| Git Integration | Fetch / Push (with dry-run preview, force push warnings, and protected bookmark detection) |
| Diff | Compare two revisions (`jj diff --from --to`) |
| Usability | Revset filtering / Text search / Adaptive status bar / Dynamic context-aware hints |

### Planned

| Area | Features |
|------|----------|
| Customization | Keybindings config / Themes |

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

**Test categories**: Unit (390), Integration (50+), Snapshot (7), Property-based (15)

## Acknowledgments

- [Jujutsu](https://github.com/jj-vcs/jj) - The modern VCS
- [tig](https://github.com/jonas/tig) - Inspiration
- [ratatui](https://ratatui.rs/) - TUI framework

## License

MIT
