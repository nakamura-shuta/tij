# Tij 開発計画（整理版）

## 1. 目的
- jj の日常操作を、安全かつ高速に行える TUI を提供する。
- 履歴編集・リモート操作を、確認導線と Undo 前提で扱える UX を維持する。

## 2. 環境情報
- テスト用リポジトリ: `https://github.com/nakamura-shuta/mytest-private-repository`
- 現在の詳細設計は `./work/docs/spec-detail/` 配下を正本とし、この `plan.md` は進行管理の要約に限定する。

## 3. 進捗サマリー

### 完了済み（主要）
- [x] Phase 1: 基盤構築
- [x] Phase 2: Log View
- [x] Phase 3: Diff View（比較Diff含む）
- [x] Phase 3.5: DAG表示
- [x] Phase 4: Status View
- [x] Phase 4.5: Blame View + Blame→Log Jump
- [x] Phase 5: Undo/Redo + Confirm + Operation History
- [x] Phase 6: Describe/Edit/New/Commit/Squash/Abandon/Split
- [x] Phase 6.7: crates.io 公開
- [x] Phase 7: Bookmark系（Create/Delete/Move/Jump/Track/View/Untrack）
- [x] Phase 8: Rebase/Absorb
- [x] Phase 9: Resolve
- [x] Phase 10: Git連携（Fetch/Push + dry-run preview + force push 警告）
- [x] Phase 11: Rebase拡張 + Describeリファクタリング
- [x] Phase 12: テスト強化（UI snapshot / integration）
- [x] Phase 14: Preview基盤 + 拡張（default ON, file summary, idle fetch）
- [x] Phase 15: Bookmark Rename/Forget + Fetch拡張 + Next/Prev

### 完了済み（最近）
- [x] Log --reversed: `V` キーで表示順トグル（v0.3.28）
- [x] Phase 16.1: `jj git push --change`（v0.3.28）
- [x] Phase 16.3: `jj git push --remote`（v0.3.28）
- [x] Phase 16.2: Duplicate（v0.3.29）
- [x] Help パネル スクロール対応（v0.3.29）
- [x] Push Bulk Options: `--all`, `--tracked`, `--deleted`（v0.3.30）
- [x] Bookmark Move to @（v0.3.30）
- [x] Phase 18.1: Diffedit（v0.3.31）
- [x] Phase 18.2: Restore file/all（v0.3.31）
- [x] Phase 18.3: Evolog View（v0.3.31）
- [x] stdin null fix: jj コマンド実行時のフリーズ防止（v0.3.31）
- [x] Phase 19: Revert（v0.3.32）
- [x] ステータスバーヒント整理（2行以内に収まるよう厳選）（v0.3.32）
- [x] Phase 20: Help パネル検索（`/` 検索、`n`/`N` ナビゲーション、ハイライト）（v0.3.33）
- [x] Phase 21: `jj git push --revisions` サポート（v0.3.34）
- [x] Phase 23: Diff Export — クリップボードコピー (`y`/`Y`) & ファイルエクスポート (`w`, git unified patch)（v0.3.35）

### 現在アクティブ
- [ ] Phase 17: パフォーマンスチューニング
- [ ] Phase 13: ワークフローエンジン（後段）

## 4. 次の実装順（推奨）

### Phase 16（優先）
#### 16.1 `jj git push --change` ✅
- [x] Push フローに `Push by change` を追加
- [x] `jj git push --change <change_id>` を実行
- [x] 既存 bookmark ベース push と共存
- [x] 通知と dry-run 表示整合を取る

#### 16.2 `jj duplicate` ✅
- [x] Log View で `Y` キーを割当
- [x] `jj duplicate <change_id>` を実行（stderr キャプチャ対応）
- [x] 成功時リフレッシュ + 複製先へフォーカス
- [x] revset 外の場合 "not in current revset" 通知
- [x] Help パネル スクロール対応（j/k/g/G）

#### 16.3 `jj git push --remote` ✅
- [x] 複数リモート時にリモート選択ダイアログ表示
- [x] `--remote <remote>` 付きで push/dry-run 実行
- [x] `--change` + `--remote` 組み合わせ対応
- [x] `push_target_remote` の全パスクリア保証

### Phase 17（性能）
#### 17.1 不要再実行削減
- [ ] View切替時の無条件 refresh 見直し
- [ ] 操作後 refresh の最小化（log/status/diff）

#### 17.2 キャッシュ改善
- [ ] Preview cache の方針明確化（容量/失効条件）
- [ ] 同一内容への再fetch削減

#### 17.3 実行方式改善
- [ ] 並列化可能な取得を整理
- [ ] 競合状態での整合ルール定義

#### 17.4 大規模リポ対応
- [ ] 計測ポイント追加（遅延可視化）
- [ ] 劣化条件でのフォールバックを定義

### Phase 13（後段）
- [ ] ワークフローエンジン基盤
- [ ] ビルトインプリセット
- [ ] ユーザー定義ワークフロー

## 5. コマンド/オプション拡張バックログ

### High
- [ ] `jj git push`: `--allow-private`, `--allow-empty-description`
  - [x] `--remote`（Phase 16.3 完了）, [x] `--change`（Phase 16.1 完了）
  - [x] `--all`, `--tracked`, `--deleted`（v0.3.30 完了）
  - [x] `--revisions`（Phase 21 完了）
- [ ] `jj git fetch`: `--tracked`, `--branch`
  - [x] `--remote`（Phase 15.3 完了）
- [x] `jj bookmark`: `move`（v0.3.30 完了）

### Medium
- [ ] `jj rebase`: `--branch`, `--revisions`, `--skip-emptied`, `--keep-divergent`
- [ ] `jj diff/show` 表示オプション拡張

### Low
- [x] `diffedit`（Phase 18.1 完了）
- [x] `restore`（Phase 18.2 完了）
- [x] `evolog`（Phase 18.3 完了）
- [x] `revert`（Phase 19 完了）
- [ ] `metaedit`, `parallelize`, `simplify-parents`, `tag`, `workspace`, `sparse`, `bisect`, `fix`, `sign/unsign`

## 6. jj log 方針
- [x] `--limit`（デフォルト表示のみ 200）
- [x] revset 指定時は無制限
- [x] `--reversed`（`V` キーで表示順トグル、v0.3.28）
- [ ] `--count`（必要性再評価）

## 7. 品質ゲート
- [ ] `cargo fmt`
- [ ] `cargo clippy` warnings 0
- [ ] `cargo test --lib`
- [ ] `cargo test --tests`
- [ ] 必要に応じて `cargo test --test ui`

## 8. 配布
- [x] crates.io: `cargo install tij`
- [x] Homebrew tap: `brew tap nakamura-shuta/tij && brew install tij`
- [x] release/homebrew 自動化
- [ ] AUR/Nix/Scoop/Winget は将来対応

## 9. 参照（詳細設計）
- Preview: `./work/docs/spec-detail/phase14-*.md`
- Bookmark/基本コマンド: `./work/docs/spec-detail/phase15-bookmark-ext-basic-commands.md`
- Push拡張: `./work/docs/spec-detail/phase10-2-push-dry-run.md` ほか

## 10. メモ
- 旧版の詳細な履歴は `./work/docs/plan.legacy.md` に退避。
- 今後は `plan.md` を「実行順と状態管理」に限定し、実装詳細は `spec-detail` に集約する。

## 11. UXアイデア（候補）

### 11.1 ヘルプ/ヒントのキーワード連動ハイライト
- [ ] `?` ヘルプ画面で `/` キーワード入力によるハイライト
- [ ] 例: `commit` 入力時に `Describe` / `Commit` / 関連キーを強調
- [ ] ステータスバーは初期段階では「一致項目を先頭表示」の軽量方式で検証
- [ ] 関連語辞書（同義語マップ）を小規模に導入
  - [ ] 例: `commit` → `describe`, `message`, `new`, `push`
  - [ ] 例: `rebase` → `move`, `insert-after`, `insert-before`

### 11.2 導入方針
- [ ] Step 1: ヘルプ画面のみ実装（低リスク）
- [ ] Step 2: 効果確認後にステータスバーへ拡張
