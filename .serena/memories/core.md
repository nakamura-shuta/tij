# Core

- Tij: Rust TUI for Jujutsu (`jj`), inspired by `tig`.
- Main source map: `src/main.rs` event loop/entrypoint; `src/app/` application state, input dispatch, refresh, actions; `src/ui/` views/components/rendering; `src/jj/` jj command execution and parsers; `src/model/` domain data.
- Docs live under `.work/docs/`; feature SoW files live under `.work/docs/spec-detail/`.
- For command-facing work, consult `mem:conventions` for established input/action/executor patterns and command-history expectations.
- For build/test commands, consult `mem:suggested_commands` and completion checks in `mem:task_completion`.
- Project guidance in `CLAUDE.md` asks agents to explain which `jj` commands should be run and why when jj operations are appropriate.