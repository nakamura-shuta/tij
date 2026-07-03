# Conventions

- Key bindings are centralized in `src/keys.rs`; help/status hints should be updated with behavior changes.
- Low-frequency Log commands are exposed through command palette (`:`). Palette entries usually dispatch by `PaletteDispatch::Command(LogCommand::...)`; view openers that retain single keys use `PaletteDispatch::Key`.
- Log input flow: `src/ui/views/log/input.rs` maps key/palette command to `LogAction`; `src/app/input.rs` routes `LogAction` to dialogs or app actions.
- Dialog callbacks live in `src/ui/components/dialog/mod.rs`; callback handling is in `src/app/actions/dialog.rs`.
- Captured jj commands belong in `src/jj/executor.rs`; interactive/inherited-stdio jj commands belong in `src/jj/interactive.rs`.
- Interactive jj methods should have an argv builder (`*_argv`) used both for spawning and `record_interactive_command`, keeping command history copy/paste accurate.
- Interactive commands suspend the TUI with `suspend_tui()` before spawning and refresh dirty log/status views after completion when repository state may change.
- Captured jj commands should use executor helpers that apply stable flags such as `--color=never`; interactive commands intentionally inherit stdio and may preserve native color/editor behavior.
- Domain models go under `src/model/`; UI state under `src/ui/` or `src/app/state.rs` depending on ownership.