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
- **リポジトリセットアップ** — ignored ファイルのコピーとコマンド実行を自動化

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

これによりワークツリーへの `cd`、タブ補完、TTY を維持したリポジトリセットアップの実行が有効になります。

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
| `copsy config repo` | リポジトリ設定を対話的に作成 |
| `copsy config global` | グローバル設定を対話的に作成 |
| `copsy setup` | 現在のワークツリーでセットアップを実行 |
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

### herdr互換のワークツリー

`--herdr` を指定すると、[herdr](https://github.com/herdrdev/herdr)のディレクトリ構造と命名規則でワークツリーを作成できます。

```sh
copsy new Feature/Login --herdr
copsy --herdr add fix/typo
copsy pr 123 --herdr
copsy --herdr                     # 対話形式でブランチを選択
```

オプションはサブコマンドの前後どちらにも指定できます。作成時は、その実行に限りcopsyの `worktree.base_dir` と `worktree.layout` より優先されます。指定しなければ従来の構造を使います。その他のコマンドは、ディレクトリ構造に関係なくGitから既存のワークツリーを取得します。

既定では、`~/dev/myapp` のクローンから `Feature/Login` を作成すると、作成先は `~/.herdr/worktrees/myapp/feature-login` になります。ルートディレクトリには `~/.config/herdr/config.toml` の `[worktrees].directory` を使います。`XDG_CONFIG_HOME` がある場合は `$XDG_CONFIG_HOME/herdr/config.toml`、`HERDR_CONFIG_PATH` がある場合は指定された設定ファイルを読み込みます。設定ファイルや項目がなければ `~/.herdr/worktrees` を使います。`~` と `~/` はホームディレクトリへ展開し、相対パスは実行時の作業ディレクトリを基準にします。不正または読み取り不能な設定ではエラーになります。

リポジトリ名は `origin` の名前ではなく、ローカルのGit共通ディレクトリから決まります。クローンを `myapp-local` に改名していれば、`myapp-local` を使います。ブランチ名はASCIIの英字を小文字にし、ASCIIの英数字以外の連続を `-` に置き換え、先頭と末尾の `-` を除去します。結果が空なら `worktree` を使います。異なるブランチが同じ名前のディレクトリになる場合は、誤ったブランチへ移動せず衝突を報告します。

herdr v0.9.1のパス規則に対応しています。herdr内（`HERDR_ENV=1`）で `new`、`add`、`pr`、対話形式の作成を実行すると、`herdr worktree open` でリポジトリの親スペースに子ワークスペースを登録します。フォーカスは移動せず、リンクされたワークツリーから実行してもメインのチェックアウトを親の基準にします。既存のワークツリーを選んだ場合も登録を再試行します。

herdr外ではパスの作成のみ行い、登録を省略した旨を表示します。登録にはherdrの実行ファイルが必要です。登録が失敗した場合は警告を表示し、Gitのワークツリーとディレクトリ移動は維持します。herdr内で `copsy add <ブランチ> --herdr` を実行すると再試行できます。リポジトリへの信頼を自動的に付与することはありません。

`copsy config global` を実行すると、`~/.config/copsy/config.toml`（`$XDG_CONFIG_HOME` に対応）にグローバル設定を対話的に作成します。ワークツリーのデフォルト作成先、ディレクトリの配置方式、未コミットの変更をデフォルトで持ち運ぶかを設定できます。

```toml
[worktree]
# ワークツリーの作成先ディレクトリ（デフォルト: メインワークツリーの親ディレクトリ）
# ~ 展開に対応
base_dir = "~/worktrees"
# ワークツリーのディレクトリ配置: "flat"（デフォルト）または "nested"
layout = "nested"
# ワークツリー移動時に未コミットの変更を持ち運ぶ
carry_changes = true
```

`base_dir` 未設定の場合、ワークツリーはメインワークツリーと同じ階層に作成されます。`flat` では `<リポジトリ名>-<ブランチ名>` という名前になり、`nested` では `<リポジトリ名>` ディレクトリの下にまとまります。この `<リポジトリ名>` がメインワークツリー自身になる場合（既定のクローンが該当）は `<リポジトリ名>-worktrees` になります。

このコマンドは初回作成専用で、既存の設定ファイルを上書きしません。

### リポジトリセットアップ

`copsy config repo` を実行すると、Git common dir の `copsy.toml` にマシン固有のリポジトリ設定を作成します。対話画面では、グローバルなワークツリー作成先と配置方式の上書き、メインワークツリーからコピーする ignored ファイル、自動的にセットアップするコマンドを選択できます。

```toml
# [worktree]
# base_dir = "~/worktrees"
# layout = "flat"
[setup]
auto = ["new"]
# command = ["npm", "install"]
copy_from_main = [
  ".env",
  ".env.local",
]
```

リポジトリの `base_dir` と `layout` はグローバル設定を上書きします。リポジトリ側で未設定の値はグローバル設定を継承します。
このコマンドは初回作成専用で、既存の設定ファイルを上書きしません。

`auto` には `new`、`add`、`pr` を指定できます。省略した場合、またはセットアップを設定しない場合は、デフォルトの no-setup 状態になります。

セットアップでは、設定されたファイルやディレクトリをコピーした後に `command` を実行します。コピー先に既存の項目がある場合は上書きしません。`copsy setup` または `--setup` で明示的に実行でき、`--no-setup` で `auto` を一時的に無効化できます。
`copsy setup` と自動セットアップはシェル統合を経由するため、パッケージマネージャーなどの対話コマンドも現在のターミナルを利用できます。

```sh
copsy new feature --setup
copsy add existing --no-setup
copsy setup
```

セットアップコマンドは任意のコードを実行できます。`copsy pr` の自動セットアップは、信頼できるリポジトリと PR に対してのみ有効にしてください。

アップグレード後はシェルを再起動するか、`copsy init zsh`／`copsy init bash` を再評価して、シェル関数を更新してください。

## ワークツリーの命名規則

ブランチ名の `/` は、どちらの配置方式でも `-` に置換されます。ディレクトリが配置方式の定める段数より深くなることはありません。

リポジトリ名は主リモートの URL から取得するため、クローン先のディレクトリを改名しても変わりません。主リモートは、`origin` があれば `origin`、なければ唯一のリモート、それもなければ `gh repo set-default` が選んだリモートです。いずれにも当たらない場合は、メインワークツリーのディレクトリ名を使います。

デフォルトの `flat` では、ワークツリーは `<リポジトリ名>-<ブランチ名>` で命名されます。

- リポジトリ `myapp`、ブランチ `feature/login` → `myapp-feature-login`
- リポジトリ `myapp`、ブランチ `fix-typo` → `myapp-fix-typo`

`nested` では `<リポジトリ名>/<ブランチ名>` に作成されるため、共有した `base_dir` の下がリポジトリごとにまとまります。

- リポジトリ `myapp`、ブランチ `feature/login` → `myapp/feature-login`
- リポジトリ `myapp`、ブランチ `fix-typo` → `myapp/fix-typo`

その `<リポジトリ名>` ディレクトリがメインワークツリー自身になる場合は、末尾に `-worktrees` が付きます。`base_dir` 未設定で、メインワークツリーのディレクトリ名がリポジトリ名と一致する場合が該当します。`git clone` の既定がこの状態です。

- メインワークツリー `~/dev/myapp`、ブランチ `fix-typo` → `~/dev/myapp-worktrees/fix-typo`
- メインワークツリー `~/dev/myapp-main`、ブランチ `fix-typo` → `~/dev/myapp/fix-typo`

入れ子のワークツリーを収めるディレクトリは、最後のワークツリーを削除した時点で削除されます。

`--herdr` を指定しない場合、作成先は常にメインワークツリーの隣（`base_dir` を設定している場合はその下）です。別のワークツリーの中から `copsy new` や `copsy add` を実行しても変わりません。

## ライセンス

MIT
