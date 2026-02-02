# Tij

**T**ext-mode **I**nterface for **J**ujutsu - A terminal user interface (TUI) for the [Jujutsu](https://github.com/jj-vcs/jj) version control system, inspired by [tig](https://github.com/jonas/tig).

## Features

- **Log View**: Browse commit history with DAG graph visualization
- **Diff View**: View changes with syntax-highlighted diffs (added/deleted/context lines)
- **Status View**: See working copy status and changed files
- **Undo/Redo**: Safely undo and redo jj operations
- **Vim-like Navigation**: Familiar keybindings (j/k, g/G, ↑/↓)
- **Revset Filtering**: Filter commits using jj's powerful revset expressions
- **Search**: Find commits by description, author, or bookmark name

## Requirements

- Rust 1.85+ (Edition 2024)
- [Jujutsu](https://github.com/jj-vcs/jj) installed and available in PATH

## Installation

### From Source

```bash
# Clone the repository
git clone https://github.com/your-username/tij.git
cd tij

# Build and install
cargo install --path .
```

### Development Build

```bash
cargo build --release
```

## Usage

Run `tij` in any Jujutsu repository:

```bash
cd /path/to/jj-repo
tij
```

Or specify a path:

```bash
tij /path/to/jj-repo
```

## Key Bindings

### Log View

| Key | Action |
|-----|--------|
| `j` / `↓` | Move down |
| `k` / `↑` | Move up |
| `g` | Go to top |
| `G` | Go to bottom |
| `Enter` | Open diff view |
| `r` | Revset filter |
| `/` | Search |
| `n` / `N` | Next/prev search result |
| `u` | Undo |
| `Ctrl+R` | Redo |
| `s` | Status view |
| `Tab` | Switch view |
| `?` | Help |
| `q` | Quit |

### Diff View

| Key | Action |
|-----|--------|
| `j` / `↓` | Scroll down |
| `k` / `↑` | Scroll up |
| `d` / `u` | Half page down/up |
| `g` / `G` | Top/bottom |
| `]` / `[` | Next/prev file |
| `q` | Back |

### Status View

| Key | Action |
|-----|--------|
| `j` / `↓` | Move down |
| `k` / `↑` | Move up |
| `Enter` | Open diff for file |
| `Tab` | Switch view |
| `q` | Quit |

### Input Mode (Revset/Search)

| Key | Action |
|-----|--------|
| `Enter` | Submit |
| `Esc` | Cancel |
| `Backspace` | Delete character |

## Revset Examples

Filter commits using jj's revset expressions:

```
# Show all commits
all()

# Show recent commits
@-..@

# Show commits by author
author(email)

# Show commits on a branch
ancestors(bookmark_name)

# Combine expressions
ancestors(main) & author(me)
```

See [jj revset documentation](https://jj-vcs.dev/latest/revsets/) for more.

## Development

```bash
# Run with cargo
cargo run

# Run tests
cargo test

# Run linter
cargo clippy

# Format code
cargo fmt
```

## Acknowledgments

- [Jujutsu](https://github.com/jj-vcs/jj) - The modern version control system
- [tig](https://github.com/jonas/tig) - Text-mode interface for Git (inspiration)
- [ratatui](https://ratatui.rs/) - Rust TUI framework
