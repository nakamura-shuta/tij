# Task Completion

- For code changes, run `cargo fmt` before finalizing.
- Run targeted tests for the touched area; broaden to `cargo test` when shared app/input/action/executor behavior changes.
- Run `cargo clippy` or `cargo clippy --fix` when requested by project guidance or when lint-sensitive changes were made.
- For UI rendering changes, run relevant snapshot tests (`cargo insta test` or targeted UI tests) and review snapshot updates deliberately.
- For docs-only SoW changes under `.work/docs/`, tests are usually not required; verify the file content and path instead.
- When using Serena onboarding memories, user can sanity-check references with `serena memories check` from the project root.