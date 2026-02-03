# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**Tij** (Text-mode Interface for Jujutsu) - A TUI application for the Jujutsu version control system, inspired by tig.

[important]
Additionally, since this project also serves as practice for using jujutsu,
please explain “which jujutsu commands should be executed and why” at points during development where using jujutsu is (or should be) appropriate.
The commands will be executed by the user.

cli reference:https://docs.jj-vcs.dev/latest/cli-reference/


## Technology Stack

- **Language**: Rust (Edition 2024)
- **TUI Framework**: ratatui 0.30
- **Terminal Backend**: crossterm 0.28
- **Error Handling**: color-eyre

## Development Commands

```bash
cargo build              # Build
cargo run                # Run
cargo test               # Test
cargo clippy             # Lint
cargo fmt                # Format
```

## Architecture

詳細は `.work/docs/architecture.md` を参照。

### ディレクトリ構造（計画）
```
src/
├── main.rs          # エントリポイント、イベントループ
├── app.rs           # App状態、画面遷移
├── keys.rs          # キーバインド定義
├── ui/              # UI層（views/, widgets/）
├── jj/              # jjコマンド実行・パース
└── model/           # データモデル
```

### 設計方針
- **jjをサブプロセス実行**: `--color=never -T <template>` で出力を安定化
- **tigライクなキーバインド**: j/k, Enter, q, Tab
- **stateless Help View**: keys.rsから動的生成
- **通知は5秒またはキー入力で消去**: タイマースレッド不使用

## Key Documents

| ドキュメント | 内容 |
|-------------|------|
| `.work/docs/spec.md` | 仕様・MVP範囲 |
| `.work/docs/plan.md` | 開発計画・進捗 |
| `.work/docs/architecture.md` | アーキテクチャ設計 |
| `.work/docs/about_jujutsu.md` | Jujutsu解説 |
| `.work/docs/spec-detail`以下に作成 | 各機能のSoW |


## Conventions

- キーバインドは `src/keys.rs` に集約
- 型の配置: ドメインモデル → `model/`、UI状態 → `ui/`
- jj実行時は必ず `--color=never` を付与

## Known Limitations

### タブ文字制約
jj出力のパースにタブ区切りを使用しているため、以下のフィールドにタブ文字が含まれると正しくパースできません:

- description（コミットメッセージ）
- author（メールアドレス）
- bookmarks

**影響**: 実際にタブ文字がこれらのフィールドに含まれることは稀ですが、もし発生した場合はパースエラーまたは不正確な表示となります。


## Build

- ビルド時はcargo fmt,cargo clippy --fix を実行すること


## References

- Jujutsu: https://github.com/jj-vcs/jj
- ratatui: https://ratatui.rs/
- tig: https://github.com/jonas/tig
- Jujutsu turorial : https://steveklabnik.github.io/jujutsu-tutorial/