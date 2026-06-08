# Tij

**T**ext-mode **I**nterface for **J**ujutsu - A TUI for the [Jujutsu](https://github.com/jj-vcs/jj) version control system, inspired by [tig](https://github.com/jonas/tig).

![Rust](https://img.shields.io/badge/rust-1.88+-orange.svg)
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

**Requirements**: [Jujutsu](https://github.com/jj-vcs/jj) >= 0.42.0 in PATH (Homebrew installs it automatically)

## Quick Start

```bash
cd /path/to/jj-repo
tij
```

Press `?` for help, `q` to quit.

## Features

| Area | Features |
|------|----------|
| Views | Log (with split-pane preview) / Diff / Status (`s`) / Help / Operation History (`o`) / Blame (with Log jump) / Bookmark (`b`) / Tag (`t`) / Workspace (`w`, `Enter` jump / `n` add / `d` forget / `r` rename, `<name>@` markers in Log) / Evolog (`L`, evolution history) / Command History (`H`, shows executed jj commands with OK/NG status) |
| History Editing | Describe (`d` quick edit / `Ctrl+E` external editor) / Edit (`e`) / New change (`c`) / New from selected (`C`) / Squash (`S`) / Abandon (`A`) / Split (`x`) / Rebase (`R`: revision/source/branch/insert-after/insert-before, with `--skip-emptied` toggle and revset input for multi-revision rebase) / Absorb (`B`) / Resolve conflicts (`X`). Low-frequency ops via command palette (`:`): interdiff / compare / duplicate / revert / simplify-parents / parallelize / metaedit / arrange / bisect / fix |
| Conflict Resolution | Resolve List View / :ours / :theirs / External merge tool / Conflict jump |
| Recovery | Undo `u` (shows undone operation detail) / Redo `Ctrl+R` — available in all mutating views (Log, Status, Bookmark, Tag, Workspace, Operation, Resolve) / Operation Restore / Restore file / Restore all |
| Bookmarks | `b` opens Bookmark View; in-view ops: `Enter` jump / `n` create / `d` delete / `r` rename / `t` track / `T` untrack / `f` forget / `m` move to @ (with backward detection). Create target defaults to Log selection (else `@`). |
| Tags | Tag View (`t`): `Enter` jump / `n` create / `d` delete. Jump supports revset expansion. |
| Git Integration | Fetch (multi-remote selection, branch-specific fetch, tracked-only fetch) / Push (with dry-run preview, force push warnings, protected bookmark detection, multi-remote selection, push-by-change, push-by-revision, bulk options: --all/--tracked/--deleted, auto-retry for private commits and empty descriptions) |
| Navigation | Next/Prev (`]`/`[` to move @ through history) / Reversed log order (`V`) |
| AI Attribution | [Agent Trace](https://github.com/cursor/agent-trace) reader: changes with AI contributions get a `[AI]` badge (`[AI?]` when anchored via git SHA) in the Log View; `:` → `show-traces` opens a Trace Detail View (time / tool+version / contributor breakdown / per-file ranges / all URLs incl. `related[]`, `y` to copy a URL); the Diff View marks AI-contributed line ranges with a `▎` gutter (color-words format, approximate by design). Reads `.agent-trace/traces.jsonl` (gitignore it so trace appends don't dirty the working copy); path configurable via `jj config set --repo tij.agent-trace.path <path>`. Reloads on `Ctrl+L`. |
| Diff | Display mode cycle (`m`: color-words → stat → git) / Copy to clipboard (`y` full / `Y` diff-only) / Export to `.patch` file (`w`, git unified format) / Stack diff (`:` → `show-stack-diff`: all diffs from the selected change up to `@` in one view, with `◉` change boundaries). Compare, interdiff, and bisect are reachable via command palette (`:`) in Log View. |
| Usability | Command palette (`:`, fuzzy-search low-frequency commands) / Current-view help (`?` shows only the current view's keys, `a` toggles all views) / Revset filtering (with count + truncation indicator) / Text search / Adaptive status bar / Dynamic context-aware hints / `--limit 200` for all queries / Startup jj version check (>= 0.42) |

## AI Attribution (Agent Trace)

Tij reads [Agent Trace](https://github.com/cursor/agent-trace) sidecar files —
an open spec for recording which code was written by AI agents — and surfaces
the attribution in four places:

| Where | What you see |
|-------|--------------|
| Log View | `[AI]` badge on changes with AI contributions (`[AI?]` when the trace is anchored via git SHA, which may point one change off in jj repos) |
| `:` → `filter-ai` | Toggle the Log View to show only AI-attributed changes (post-filter over the loaded set; title shows `[AI] (N)`) |
| `:` → `ai-summary` | One-line AI contribution summary over the loaded changes: `AI 12/40 (30%) · [AI] 9 [AI?] 3 · models: …` |
| `:` → `ai-report` | Write a Markdown AI-attribution report (`agent-trace-report.md` in the workspace root): summary + a per-change table (change / confidence / description / model / files+ranges / session URL), plus an "Orphaned traces" section when any trace anchors a revision outside the loaded changes. Same loaded-set scope as `ai-summary`; overwrites on each run |
| `:` → `ai-orphans` | One-line count of orphaned traces — traces anchored to a revision matching none of the loaded changes (out of the `--limit` window, rebased, or abandoned): `Orphaned traces: 2 (jj 1, git 1)` |
| `:` → `show-traces` | Trace Detail View for the selected change: per record, time / tool+version / contributor breakdown / per-file ranges, and every URL (conversation + `related[]`); `j/k` to scroll, `y` to copy the URL on the cursor row |
| Diff View | `▎` gutter on AI-contributed line ranges (color-words format; approximate by design) |
| Blame View (`a`) | `[AI]` / `[AI?]` column on each hunk-head line whose originating change has AI contributions (change-level, so a human-written line in an AI change is also marked — use the Diff gutter for line-precise attribution) |

### Quick start

1. Have an agent host write traces — e.g. hook [the reference implementation](https://github.com/cursor/agent-trace/tree/main/reference) into Claude Code / Cursor, or write a small jj-aware hook that records `vcs.type: "jj"` with the change ID of `@` (recommended: change IDs survive rebases, so badges show as confirmed `[AI]`)
2. **Gitignore the sidecar** (`/.agent-trace/`) — otherwise every trace append dirties the working copy
3. Open tij: badged changes appear immediately; `Ctrl+L` reloads after new traces

No trace file? The feature stays completely silent — zero UI change.

```bash
# Optional: store the sidecar elsewhere (repo or user scope)
jj config set --repo tij.agent-trace.path "path/to/traces.jsonl"
```

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

**Test categories**: Unit (741), Integration (86), Snapshot (20), Property-based (15)

## Acknowledgments

- [Jujutsu](https://github.com/jj-vcs/jj) - The modern VCS
- [tig](https://github.com/jonas/tig) - Inspiration
- [ratatui](https://ratatui.rs/) - TUI framework

## License

MIT
