<p align="center">
  <img src="assets/logo.png" alt="copsy" width="40%">
</p>

<h1 align="center">copsy</h1>

<p align="center">
  Git worktree をかんたんに作成・切り替え・管理できる CLI ツール。インタラクティブなブランチ選択、PR チェックアウト、エディタ/AI ツールの起動をサポートします。
</p>

[English README](README.md)

## 特徴

- **インタラクティブなブランチ選択** — ローカル・リモートブランチ、既存ワークツリーをファジー検索で選択
- **PR チェックアウト** — GitHub PR 番号や URL から直接ワークツリーを作成
- **シェル統合** — ワークツリーへの `cd` が実際に行われる（パス表示だけではない）
- **エディタ/AI 起動** — 切り替え後に VS Code、Cursor、Claude Code、Codex CLI を自動起動
- **タブ補完** — zsh/bash でサブコマンド・ブランチ名・ワークツリー名を補完
- **色分け表示** — リポジトリ・ワークツリー・ローカルブランチ・リモートブランチをひと目で区別
- **ワークツリー作成先の設定** — 設定ファイルで作成ディレクトリを指定可能

## 必要なもの

- Rust（edition 2024）
- Git
- [gh](https://cli.github.com/)（GitHub CLI）— `copsy pr` で必要

## インストール

```sh
cargo install --path .
```

### シェル統合

`~/.zshrc` に追加:

```sh
eval "$(copsy init zsh)"
```

bash の場合は `~/.bashrc` に追加:

```sh
eval "$(copsy init bash)"
```

これによりワークツリーへの `cd` とタブ補完が有効になります。

## 使い方

### インタラクティブモード

```sh
copsy
```

引数なしで実行すると、全ブランチとワークツリーのファジー検索リストが表示されます:

- **既存ワークツリー** → そのディレクトリに移動
- **ローカル/リモートブランチ** → 新しいワークツリーを作成して移動

**Esc** でキャンセルできます。

### コマンド一覧

| コマンド | 説明 |
|---|---|
| `copsy new <branch>` | 新規ブランチでワークツリーを作成 |
| `copsy add <branch>` | 既存ブランチでワークツリーを作成 |
| `copsy switch` (`sw`) | 既存ワークツリーをファジー選択して移動 |
| `copsy remove` (`rm`) | ワークツリーをファジー選択して削除 |
| `copsy list` (`ls`) | 全ワークツリーを一覧表示 |
| `copsy status` | 各ワークツリーの `git status` を表示 |
| `copsy close` | 現在のワークツリーを閉じてメインに戻る |
| `copsy pr [対象]` | PR をワークツリーとしてチェックアウト（対象省略で対話選択） |
| `copsy init <shell>` | シェル統合スクリプトを出力（`zsh` または `bash`） |

### PR チェックアウト

```sh
copsy pr 123                                       # PR 番号で指定
copsy pr https://github.com/owner/repo/pull/123    # URL で指定
copsy pr                                           # 対話的に選択
```

PR のブランチを fetch し、ワークツリーを作成（または既存のものに切り替え）します。

### 起動フラグ

エディタや AI ツールを切り替え後に起動します。複数同時に指定可能:

```sh
copsy --claude              # Claude Code を起動
copsy --codex               # Codex CLI を起動
copsy --code                # VS Code で開く
copsy --cursor              # Cursor で開く
copsy --open "my-command"   # 任意のコマンドを実行

copsy add feature --claude --code   # ワークツリー作成 + VS Code + Claude
copsy pr 42 --cursor                # PR #42 チェックアウト + Cursor
```

| フラグ | 短縮形 | ツール |
|---|---|---|
| `--claude` | `-c` | Claude Code |
| `--codex` | `-x` | Codex CLI |
| `--code` | | VS Code |
| `--cursor` | | Cursor |
| `--open <cmd>` | | 任意のコマンド |

## 設定

設定ファイル: `~/.config/copsy/config.toml`（`$XDG_CONFIG_HOME` に対応）

```toml
[worktree]
# ワークツリーの作成先ディレクトリ（デフォルト: リポジトリの親ディレクトリ）
# ~ 展開に対応
base_dir = "~/worktrees"
```

`base_dir` 未設定の場合、ワークツリーはリポジトリと同じ階層に `<リポジトリ名>-<ブランチ名>` の形式で作成されます。

## ワークツリーの命名規則

ワークツリーは `<リポジトリ名>-<ブランチ名>` で命名され、ブランチ名の `/` は `-` に置換されます。

例:
- リポジトリ `myapp`、ブランチ `feature/login` → `myapp-feature-login`
- リポジトリ `myapp`、ブランチ `fix-typo` → `myapp-fix-typo`

## ライセンス

MIT
