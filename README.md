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
# From crates.io (recommended)
cargo install tij

# From source
git clone https://github.com/nakamura-shuta/tij.git
cd tij && cargo install --path .
```

**Requirements**: Rust 1.85+, [Jujutsu](https://github.com/jj-vcs/jj) in PATH

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
| Views | Log / Diff / Status / Help / Operation History / Blame |
| History Editing | Describe (multi-line) / Edit / New / New from selected / Commit / Squash / Abandon / Split / Rebase / Absorb |
| Conflict Resolution | Resolve List View / :ours / :theirs / External merge tool / Conflict jump |
| Recovery | Undo / Redo / Operation Restore |
| Bookmarks | Create / Move / Delete (multi-select) / Track remote / Jump |
| Git Integration | Fetch / Push (with confirmation dialog) |
| Usability | Revset filtering / Text search / Adaptive status bar |

### Planned

| Area | Features |
|------|----------|
| Views | Bookmark View |
| Safety | Push preview (dry-run) / Force push warnings |
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

## Acknowledgments

- [Jujutsu](https://github.com/jj-vcs/jj) - The modern VCS
- [tig](https://github.com/jonas/tig) - Inspiration
- [ratatui](https://ratatui.rs/) - TUI framework

## License

MIT
