# sav

`sav` is a lightweight document version control system written in Rust. It takes snapshots of all files in the current directory (excluding specified patterns), allowing you to view commit history logs, check unified diffs, and restore files or the entire directory.

---

## Features

1. **Subcommand-less Snapshots**:
   - You can save a snapshot by running `sav` directly without specifying the `save` subcommand (running `sav save` is still supported).
2. **Auto-generated Comments & Interactive Confirmation**:
   - If no memo (comment) is specified when saving, `sav` generates a summary of changes (added/updated/removed files) and prompts: `Is this comment okay? (y/n) [y]: `.
   - If you decline (`n`), you can enter a custom comment directly from stdin.
3. **Local Configuration File (`.sav.toml`)**:
   - Allows customization of the SQLite database file path (`db_path`) and glob-style exclusion patterns (`exclude`) on a per-project basis.
4. **Diff & Restore**:
   - Compare local files with any past snapshot using `sav diff <snapshot_name>`.
   - Restore a specific file or all files using `sav restore <snapshot_name> [file_path]`.
5. **Automatic Safety Restore (Auto-save)**:
   - When restoring, if there are uncommitted modifications in your workspace, `sav` automatically takes a silent snapshot (`[Auto-save] Before restoring to <snapshot>`) beforehand to guarantee your work is never lost.

---

## Installation

### From crates.io
You can install `sav` directly from crates.io:

```bash
cargo install sav-vcs
```

*(This will install the binary as `sav`)*

### From Source
Alternatively, you can build from source:

```bash
cargo build --release
```

The compiled binary will be generated at `target/release/sav`.

---

## Usage

### 1. Initialize Repository (`sav init`)

Run the following command in your project directory:

```bash
sav init
```

You will be prompted to enter the database path:
- Press Enter to use the default path `.sav.db`.
- This creates the `.sav.toml` configuration file and initializes the empty SQLite database tables at the specified path.

### 2. Save Snapshot (`sav` or `sav save`)

Save the current changes by running `sav` directly, or using the `save` subcommand:

```bash
# Prompt to confirm/edit the auto-generated comment
sav

# Save with a custom comment directly
sav -m "My snapshot message"
# or
sav save --memo "My snapshot message"
```

### 3. View Log History (`sav log`)

Display a list of all saved snapshots:

```bash
sav log
```

### 4. Show Diffs (`sav diff`)

Compare the current local files with a specific snapshot:

```bash
sav diff <snapshot_name>
```

### 5. Restore Files (`sav restore`)

Restore files or the entire directory to the state of a specific snapshot:

```bash
# Restore all files in the snapshot
sav restore <snapshot_name>

# Restore a specific file only
sav restore <snapshot_name> <file_path>
```

*Note: If there are uncommitted changes in the repository, `sav` automatically saves them in a silent snapshot before restoring.*

---

## Configuration File (`.sav.toml`)

Created automatically during initialization, this file lets you customize `sav`'s behavior:

```toml
db_path = ".sav.db"
exclude = [
    ".git/**",
    ".sav.db",
    "target/**", # Exclude build output artifacts
]
```

- `db_path`: File path to the SQLite database.
- `exclude`: A list of glob-style patterns specifying which files and folders to exclude from snapshots.

---

# 日本語版

`sav` は、個人向けの軽量ドキュメントバージョン管理システムです。現在のディレクトリ内のすべてのファイルをスナップショットとして保存し、履歴ログの閲覧、差分の表示、特定のファイルやディレクトリ全体の復元を行うことができます。

---

## 主な機能

1. **サブコマンド不要のスナップショット保存**:
   - `sav` を直接実行するだけで、現在のディレクトリの状態をスナップショットとして保存できます（`sav save` も使用可能です）。
2. **自動コメント生成と対話型確認**:
   - スナップショット保存時にメモ（コメント）が指定されていない場合、自動生成されたコメント（追加/変更/削除されたファイルの一覧）を提示し、それを使用するかどうかを確認します。拒否（`n`）した場合は、手動でカスタムコメントを入力できます。
3. **ローカル設定ファイル (`.sav.toml`)**:
   - データベースの保存先パス（`db_path`）と除外パターン（`exclude`、Glob形式）をプロジェクト単位で柔軟にカスタマイズできます。
4. **差分と復元**:
   - 任意の過去スナップショットとの差分を表示したり（`sav diff`）、スナップショットから特定のファイルまたはプロジェクト全体を復元（`sav restore`）できます。
5. **復元時の自動安全バックアップ（自動セーブ）**:
   - スナップショットを復元する際、ワークスペース内に未保存の変更がある場合、自動的にバックアップ（`[自動セーブ] <復元先> への復元前`）を保存してから復元を行います。これにより、書きかけのデータが上書きされて失われるのを防ぎます。

---

## インストール

### crates.io からインストール
crates.io から直接インストールできます：

```bash
cargo install sav-vcs
```

*（コマンド名は `sav` としてインストールされます）*

### ソースコードからビルド
または、ソースコードからビルドすることも可能です：

```bash
cargo build --release
```

ビルドされたバイナリは `target/release/sav` に生成されます。

---

## 使い方

### 1. リポジトリの初期化 (`sav init`)

プロジェクトディレクトリで以下のコマンドを実行します。

```bash
sav init
```

実行すると、データベースの保存先パスを尋ねられます。
- エンターキーを押すとデフォルトの `.sav.db` に設定されます。
- 設定ファイル `.sav.toml` が作成され、指定したパスに SQLite データベースが初期化されます。

### 2. スナップショットの保存 (`sav` または `sav save`)

現在の変更を保存するには、サブコマンドなしで直接実行するか、`save` コマンドを使用します。

```bash
# コメントを対話式で決定して保存
sav

# コメントを直接指定して保存
sav -m "特定の変更メモ"
# または
sav save --memo "特定の変更メモ"
```

### 3. 履歴の表示 (`sav log`)

保存されたスナップショットの履歴一覧を表示します。

```bash
sav log
```

### 4. 差分の表示 (`sav diff`)

現在のローカルファイルと特定のスナップショットとの差分を表示します。

```bash
sav diff <スナップショット名>
```

### 5. 復元 (`sav restore`)

指定したスナップショットからファイルまたはプロジェクト全体を復元します。

```bash
# プロジェクト全体を復元
sav restore <スナップショット名>

# 特定のファイルのみを復元
sav restore <スナップショット名> <ファイルパス>
```

*※ 未コミットの変更がある場合、復元を実行する前に自動的にバックアップスナップショットが作成されます。*

---

## 設定ファイル (`.sav.toml`)

初期化時に作成される設定ファイルで、挙動をカスタマイズできます。

```toml
db_path = ".sav.db"
exclude = [
    ".git/**",
    ".sav.db",
    "target/**", # コンパイル生成物を除外する場合
]
```

- `db_path`: データベースファイルへのパス。
- `exclude`: スナップショット対象から除外するファイルの Glob パターンリスト。

---

