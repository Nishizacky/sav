//! `sav-vcs` is a lightweight document version control system.
//! It allows users to take snapshots of all files in the current directory,
//! excluding patterns defined in a local `.sav.toml` configuration file.
//! Snapshots are compressed with zstd and stored in a local SQLite database.

use chrono::Local;
use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};
use rand::seq::IndexedRandom;
use rusqlite::{Connection, Result, params};
use std::env;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

const CONFIG_FILE: &str = ".sav.toml";

const DEFAULT_DB_NAME: &str = ".sav.db";

/// Configuration schema for `sav` stored in `.sav.toml`.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
struct Config {
    /// File path to the SQLite database.
    db_path: String,
    /// List of glob patterns for files and directories to exclude from snapshots.
    exclude: Vec<String>,
}

/// Helper function to check if the active language environment is Japanese.
fn is_japanese() -> bool {
    env::var("LANG")
        .map(|l| l.starts_with("ja"))
        .unwrap_or(false)
}

/// Loads and parses the configuration from `.sav.toml`.
///
/// Returns an error if the configuration file is missing or contains invalid TOML.
fn load_config() -> std::result::Result<Config, String> {
    if !Path::new(CONFIG_FILE).exists() {
        let err_msg = if is_japanese() {
            format!("エラー: {} がありません。まずは 'sav init' を実行して初期化してください。", CONFIG_FILE)
        } else {
            format!("Error: {} does not exist. Please run 'sav init' first to initialize.", CONFIG_FILE)
        };
        return Err(err_msg);
    }
    let mut file = File::open(CONFIG_FILE).map_err(|e| format!("Failed to open {}: {}", CONFIG_FILE, e))?;
    let mut contents = String::new();
    file.read_to_string(&mut contents).map_err(|e| format!("Failed to read {}: {}", CONFIG_FILE, e))?;
    let config: Config = toml::from_str(&contents).map_err(|e| format!("Failed to parse {}: {}", CONFIG_FILE, e))?;
    Ok(config)
}

/// Checks if a file or directory path should be excluded based on configured patterns.
///
/// Compares the relative path, its ancestor directories, and the database path.
fn is_excluded(path: &Path, config: &Config, patterns: &[glob::Pattern]) -> bool {
    let rel_path = path.strip_prefix("./").unwrap_or(path);
    let path_str = rel_path.to_string_lossy().to_string();

    // Check if it is the database file itself
    if rel_path == Path::new(&config.db_path) {
        return true;
    }

    for pattern in patterns {
        if pattern.matches(&path_str) {
            return true;
        }
        for ancestor in rel_path.ancestors() {
            let ancestor_str = ancestor.to_string_lossy().to_string();
            if !ancestor_str.is_empty() && pattern.matches(&ancestor_str) {
                return true;
            }
        }
    }
    false
}

// ランダムネーム用の辞書
const ADJECTIVES: &[&str] = &[
    "brave", "quiet", "hollow", "swift", "amber", "gentle", "silver", "cold", "bright", "hidden",
    "fierce", "silent", "golden", "shadowy", "ancient", "vibrant", "distant", "frosty", "mystic",
    "rapid", "calm", "crimson", "wild", "secret", "glowing", "haze", "bold", "smooth", "cozy",
    "noble",
];
const NOUNS: &[&str] = &[
    "sushi",
    "sashimi",
    "ramen",
    "udon",
    "soba",
    "onigiri",
    "tempura",
    "yakitori",
    "gyoza",
    "tonkatsu",
    "natoh",
    "mochi",
    "dango",
    "katsudon",
    "tohfu",
    "kinpira",
    "tororo",
    "yohkan",
    "karaage",
    "sukiyaki",
    "shabushabu",
    "okonomiyaki",
    "takoyaki",
    "misoshiru",
    "gyudon",
    "oyakodon",
    "chawanmushi",
    "hiyayakko",
    "chikuzenni",
    "taiyaki",
];

/// Command line interface parser structure for `sav`.
#[derive(Parser)]
#[command(name = "sav")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    #[arg(short, long)]
    memo: Option<String>,
}

/// Available subcommands for `sav`.
#[derive(Subcommand)]
enum Commands {
    Init,
    Save {
        #[arg(short, long)]
        memo: Option<String>,
    },
    Log,
    /// 指定したスナップショット（または特定ファイル）を復元
    Restore {
        /// スナップショット名 (例: yaki-sushi)
        name: String,
        /// 特定のファイルのみを復元したい場合に指定（相対パス）
        path: Option<String>,
    },
    /// 現在のファイルと特定スナップショットとの差分を表示
    Diff {
        /// スナップショット名 (例: yaki-sushi)
        name: String,
    },
}

/// Main entry point of the command line utility.
fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // 1. 環境変数 LANG を見て日本語か英語かを判定
    let is_ja = is_japanese();

    // 2. clap のヘルプメッセージを動的に書き換える
    let mut cmd = Cli::command();
    if is_ja {
        cmd = cmd.about("個人向けの軽量ドキュメントバージョン管理システム");
        // 各サブコマンドのヘルプを日本語に置換
        cmd = cmd.mut_subcommand("init", |s| s.about("データベースを初期化します"));
        cmd = cmd.mut_subcommand("save", |s| {
            s.about("現在のドキュメントのスナップショットを保存します")
        });
        cmd = cmd.mut_subcommand("log", |s| s.about("スナップショットの履歴を表示します"));
        cmd = cmd.mut_subcommand("restore", |s| {
            s.about("指定したスナップショット、または特定のファイルを復元します")
        });
        cmd = cmd.mut_subcommand("diff", |s| {
            s.about("現在のローカルファイルと指定スナップショットの差分を表示します")
        });
        cmd = cmd.mut_arg("memo", |a| {
            a.help("スナップショットのメモ（サブコマンドを省略した場合に適用されます）")
        });
    } else {
        cmd = cmd.about("A lightweight document version control system");
    }

    // 動的ヘルプを適用してパース
    let matches = cmd.get_matches();
    let cli = Cli::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());

    match &cli.command {
        Some(Commands::Init) => init_db()?,
        Some(Commands::Save { memo }) => save_snapshot(memo.as_deref())?,
        Some(Commands::Log) => show_log()?,
        Some(Commands::Restore { name, path }) => restore_snapshot(name, path.as_deref())?,
        Some(Commands::Diff { name }) => diff_snapshot(name)?,
        None => save_snapshot(cli.memo.as_deref())?,
    }

    Ok(())
}

/// Initializes the `.sav.toml` config file and database tables.
fn init_db() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let is_ja = is_japanese();

    // Check if configuration file already exists
    if Path::new(CONFIG_FILE).exists() {
        if is_ja {
            eprintln!("エラー: すでに {} は存在します。", CONFIG_FILE);
        } else {
            eprintln!("Error: {} already exists.", CONFIG_FILE);
        }
        return Ok(());
    }

    // Interview the user for database path
    if is_ja {
        print!("データベースの保存先パスを入力してください (デフォルト: {}): ", DEFAULT_DB_NAME);
    } else {
        print!("Enter database path (default: {}): ", DEFAULT_DB_NAME);
    }
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let mut db_path = input.trim().to_string();
    if db_path.is_empty() {
        db_path = DEFAULT_DB_NAME.to_string();
    }

    // Automatically create parent directories if needed
    let db_path_obj = Path::new(&db_path);
    if let Some(parent) = db_path_obj.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }

    // Create .sav.toml config file
    let config = Config {
        db_path: db_path.clone(),
        exclude: vec![".git/**".to_string(), db_path.clone()],
    };
    let toml_str = toml::to_string_pretty(&config)?;
    fs::write(CONFIG_FILE, toml_str)?;

    if is_ja {
        println!("設定ファイル {} を作成しました。", CONFIG_FILE);
    } else {
        println!("Created configuration file {}.", CONFIG_FILE);
    }

    // Initialize SQLite database
    let conn = Connection::open(&db_path)?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS blobs (hash TEXT PRIMARY KEY, data BLOB)",
        [],
    )?;
    conn.execute("CREATE TABLE IF NOT EXISTS snapshots (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT UNIQUE, memo TEXT, created TEXT)", [])?;
    conn.execute("CREATE TABLE IF NOT EXISTS files (snapshot_id INTEGER REFERENCES snapshots(id), path TEXT, hash TEXT REFERENCES blobs(hash), PRIMARY KEY (snapshot_id, path))", [])?;

    if is_ja {
        println!("空の sav リポジトリを初期化しました: {}", db_path);
    } else {
        println!("Initialized empty sav repository in {}", db_path);
    }

    Ok(())
}

/// Generates a randomized, unique snapshot name using adjectives and nouns.
fn generate_unique_name(conn: &Connection) -> Result<String> {
    let mut rng = rand::rng();
    loop {
        let adj = ADJECTIVES.choose(&mut rng).unwrap();
        let noun = NOUNS.choose(&mut rng).unwrap();
        let name = format!("{}-{}", adj, noun);
        let mut stmt = conn.prepare("SELECT 1 FROM snapshots WHERE name = ?")?;
        if !stmt.exists(params![name])? {
            return Ok(name);
        }
    }
}

/// Saves a new snapshot of the current tracked files.
///
/// Prompts the user to confirm/edit the auto-generated comment if no memo is specified.
fn save_snapshot(memo: Option<&str>) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let config = load_config()?;
    let db_path = &config.db_path;

    if !Path::new(db_path).exists() {
        let is_ja = is_japanese();
        if is_ja {
            eprintln!("エラー: データベースファイル '{}' がありません。まずは 'sav init' を実行して初期化してください。", db_path);
        } else {
            eprintln!("Error: Database file '{}' does not exist. Please run 'sav init' first to initialize.", db_path);
        }
        return Ok(());
    }

    let mut conn = Connection::open(db_path)?;
    let tx = conn.transaction()?;

    // --- 1. 前回の最新スナップショットのファイル構成を取得 ---
    use std::collections::HashMap;
    let mut last_files: HashMap<String, String> = HashMap::new(); // path -> hash
    {
        let mut stmt = tx.prepare(
            "SELECT f.path, f.hash FROM files f 
             WHERE f.snapshot_id = (SELECT id FROM snapshots ORDER BY id DESC LIMIT 1)",
        )?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            last_files.insert(row.get(0)?, row.get(1)?);
        }
    }

    // 現在の対象ファイルを集める
    let mut target_files = Vec::new();
    let patterns: Vec<glob::Pattern> = config
        .exclude
        .iter()
        .filter_map(|p| glob::Pattern::new(p).ok())
        .collect();

    collect_files(Path::new("."), &mut target_files, &config, &patterns);
    if target_files.is_empty() && last_files.is_empty() {
        let is_ja = is_japanese();
        if is_ja {
            println!("対象となるドキュメントファイルが見つかりません。");
        } else {
            println!("No target document files found.");
        }
        return Ok(());
    }

    // --- 2. 現在のファイルを処理しつつ、変更を検知 ---
    let mut current_files = HashMap::new(); // 今回保存するファイルのパスとハッシュ
    let mut change_logs = Vec::new();

    for path in target_files {
        let mut file = File::open(&path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;

        let hash = blake3::hash(&buffer).to_hex().to_string();
        let path_str = path.to_string_lossy().to_string();
        current_files.insert(path_str.clone(), hash.clone());

        // データベースに実体がない場合のみ、zstd圧縮して保存
        let mut stmt = tx.prepare("SELECT 1 FROM blobs WHERE hash = ?")?;
        if !stmt.exists(params![hash])? {
            let compressed_data = zstd::encode_all(&buffer[..], 3)?;
            tx.execute(
                "INSERT INTO blobs (hash, data) VALUES (?, ?)",
                params![hash, compressed_data],
            )?;
        }

        // 差分検知 (Added か Updated か)
        match last_files.get(&path_str) {
            None => change_logs.push(format!("added: {}", path_str)),
            Some(last_hash) => {
                if *last_hash != hash {
                    change_logs.push(format!("updated: {}", path_str));
                }
            }
        }
    }

    // --- 3. 削除されたファイル (Removed) の検知 ---
    for last_path in last_files.keys() {
        if !current_files.contains_key(last_path) {
            change_logs.push(format!("removed: {}", last_path));
        }
    }

    // 何も変更がない場合は、無駄なスナップショットを作らずに終了する
    if change_logs.is_empty() {
        let is_ja = is_japanese();
        if is_ja {
            println!("変更がありません。スナップショットの保存をスキップしました。");
        } else {
            println!("No changes detected. Skipping snapshot saving.");
        }
        return Ok(());
    }

    // --- 4. メモの決定（指定がなければ自動生成して確認） ---
    let final_memo = match memo {
        Some(m) => m.to_string(),
        None => {
            let auto_memo = change_logs.join(", ");
            let is_ja = is_japanese();
            if is_ja {
                println!("自動生成されたコメント: \"{}\"", auto_memo);
                print!("このコメントでよろしいですか？ (y/n) [y]: ");
            } else {
                println!("Auto-generated comment: \"{}\"", auto_memo);
                print!("Is this comment okay? (y/n) [y]: ");
            }
            io::stdout().flush()?;
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let ans = input.trim().to_lowercase();

            if ans == "n" || ans == "no" {
                if is_ja {
                    print!("コメントを入力してください: ");
                } else {
                    print!("Enter custom comment: ");
                }
                io::stdout().flush()?;
                let mut custom_input = String::new();
                io::stdin().read_line(&mut custom_input)?;
                let custom_memo = custom_input.trim().to_string();
                if custom_memo.is_empty() {
                    auto_memo
                } else {
                    custom_memo
                }
            } else {
                auto_memo
            }
        }
    };

    // スナップショットレコードの作成
    let name = generate_unique_name(&tx)?;
    let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    tx.execute(
        "INSERT INTO snapshots (name, memo, created) VALUES (?, ?, ?)",
        params![name, final_memo, now],
    )?;
    let snapshot_id = tx.last_insert_rowid();

    // ファイルメタデータの紐付け
    for (path_str, hash) in current_files {
        tx.execute(
            "INSERT INTO files (snapshot_id, path, hash) VALUES (?, ?, ?)",
            params![snapshot_id, path_str, hash],
        )?;
    }

    tx.commit()?;
    println!("Saved snapshot: {} ({})", name, final_memo);
    Ok(())
}

/// Recursively walks directories to collect paths of all files that are not excluded.
fn collect_files(dir: &Path, files: &mut Vec<PathBuf>, config: &Config, patterns: &[glob::Pattern]) {
    if is_excluded(dir, config, patterns) {
        return;
    }
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if is_excluded(&path, config, patterns) {
                continue;
            }
            if path.is_dir() {
                collect_files(&path, files, config, patterns);
            } else {
                let clean_path = path.strip_prefix("./").unwrap_or(&path).to_path_buf();
                files.push(clean_path);
            }
        }
    }
}

/// Displays a history log of all saved snapshots.
fn show_log() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let config = load_config()?;
    let db_path = &config.db_path;

    if !Path::new(db_path).exists() {
        let is_ja = is_japanese();
        if is_ja {
            eprintln!("エラー: データベースファイル '{}' がありません。まずは 'sav init' を実行して初期化してください。", db_path);
        } else {
            eprintln!("Error: Database file '{}' does not exist. Please run 'sav init' first to initialize.", db_path);
        }
        return Ok(());
    }

    let conn = Connection::open(db_path)?;
    let mut stmt = conn.prepare("SELECT name, created, memo FROM snapshots ORDER BY id DESC")?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;

    println!("{:<20} {:<20} {}", "NAME", "CREATED", "MEMO");
    println!("{}", "-".repeat(60));
    for row in rows.flatten() {
        println!("{:<20} {:<20} {}", row.0, row.1, row.2);
    }
    Ok(())
}

/// Restores all files or a specific file from a saved snapshot.
fn restore_snapshot(name: &str, target_path: Option<&str>) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let config = load_config()?;
    let db_path = &config.db_path;

    if !Path::new(db_path).exists() {
        let is_ja = is_japanese();
        if is_ja {
            eprintln!("エラー: データベースファイル '{}' がありません。まずは 'sav init' を実行して初期化してください。", db_path);
        } else {
            eprintln!("Error: Database file '{}' does not exist. Please run 'sav init' first to initialize.", db_path);
        }
        return Ok(());
    }

    let conn = Connection::open(db_path)?;

    // スナップショットの存在確認とID取得
    let mut stmt = conn.prepare("SELECT id FROM snapshots WHERE name = ?")?;
    let snapshot_id: i64 = match stmt.query_row(params![name], |row| row.get(0)) {
        Ok(id) => id,
        Err(_) => {
            let is_ja = is_japanese();
            if is_ja {
                eprintln!("エラー: スナップショット '{}' が見つかりません。", name);
            } else {
                eprintln!("Error: Snapshot '{}' not found.", name);
            }
            return Ok(());
        }
    };

    if let Some(specific_path) = target_path {
        let clean_path = Path::new(specific_path).strip_prefix("./").unwrap_or(Path::new(specific_path));
        let clean_path_str = clean_path.to_string_lossy().to_string();

        // 【ファイル個別復元】
        // 1. 指定ファイルデータを取得
        let mut stmt = conn.prepare(
            "SELECT b.data FROM files f JOIN blobs b ON f.hash = b.hash WHERE f.snapshot_id = ? AND f.path = ?"
        )?;
        let compressed_blob: Vec<u8> =
            match stmt.query_row(params![snapshot_id, &clean_path_str], |row| row.get(0)) {
                Ok(data) => data,
                Err(_) => {
                    let is_ja = is_japanese();
                    if is_ja {
                        eprintln!(
                            "エラー: スナップショット '{}' 内にファイル '{}' が見つかりません。",
                            name, clean_path_str
                        );
                    } else {
                        eprintln!(
                            "Error: File '{}' not found in snapshot '{}'.",
                            clean_path_str, name
                        );
                    }
                    return Ok(());
                }
            };

        // 解凍して上書き
        let decompressed = zstd::decode_all(&compressed_blob[..])?;
        if let Some(parent) = Path::new(&clean_path_str).parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = File::create(&clean_path_str)?;
        file.write_all(&decompressed)?;

        println!(
            "Restored single file '{}' from snapshot '{}'.",
            clean_path_str, name
        );
        let is_ja = is_japanese();
        if is_ja {
            println!("ヒント: 変更を確定させる場合は 'sav save' を実行してください。");
        } else {
            println!("Tip: Run 'sav save' to commit the changes.");
        }
    } else {
        // 【ディレクトリ丸ごと復元】
        let mut stmt = conn.prepare(
            "SELECT f.path, b.data FROM files f JOIN blobs b ON f.hash = b.hash WHERE f.snapshot_id = ?"
        )?;
        let file_rows = stmt.query_map(params![snapshot_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?))
        })?;

        for row in file_rows.flatten() {
            let (path_str, compressed_blob) = row;
            let decompressed = zstd::decode_all(&compressed_blob[..])?;
            let path = Path::new(&path_str);

            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut file = File::create(path)?;
            file.write_all(&decompressed)?;
        }
        println!("Restored all files from snapshot '{}'.", name);
    }

    Ok(())
}

/// Displays the unified diff between the current workspace files and a snapshot.
fn diff_snapshot(name: &str) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let config = load_config()?;
    let db_path = &config.db_path;

    if !Path::new(db_path).exists() {
        let is_ja = is_japanese();
        if is_ja {
            eprintln!("エラー: データベースファイル '{}' がありません。まずは 'sav init' を実行して初期化してください。", db_path);
        } else {
            eprintln!("Error: Database file '{}' does not exist. Please run 'sav init' first to initialize.", db_path);
        }
        return Ok(());
    }

    let conn = Connection::open(db_path)?;

    // スナップショットIDを取得
    let mut stmt = conn.prepare("SELECT id FROM snapshots WHERE name = ?")?;
    let snapshot_id: i64 = match stmt.query_row(params![name], |row| row.get(0)) {
        Ok(id) => id,
        Err(_) => {
            let is_ja = is_japanese();
            if is_ja {
                eprintln!("エラー: スナップショット '{}' が見つかりません。", name);
            } else {
                eprintln!("Error: Snapshot '{}' not found.", name);
            }
            return Ok(());
        }
    };

    // スナップショットに含まれる全ファイルを取得
    let mut stmt = conn.prepare(
        "SELECT f.path, b.data FROM files f JOIN blobs b ON f.hash = b.hash WHERE f.snapshot_id = ?"
    )?;
    let file_rows = stmt.query_map(params![snapshot_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?))
    })?;

    for row in file_rows.flatten() {
        let (path_str, compressed_blob) = row;
        let old_content_bytes = zstd::decode_all(&compressed_blob[..])?;
        let old_content = String::from_utf8_lossy(&old_content_bytes);

        // 現在のローカルファイルの内容を読み込む（削除されている場合は空扱い）
        let current_content = if Path::new(&path_str).exists() {
            fs::read_to_string(&path_str).unwrap_or_default()
        } else {
            String::new()
        };

        // similar クレートを使った unified diff の生成
        let diff = similar::TextDiff::from_lines(&old_content, &current_content);

        let has_changes = diff
            .iter_all_changes()
            .any(|c| c.tag() != similar::ChangeTag::Equal);
        if has_changes {
            println!("\n--- [Snapshot: {}] {}", name, path_str);
            println!("+++ [Current]       {}", path_str);

            for change in diff.iter_all_changes() {
                let sign = match change.tag() {
                    similar::ChangeTag::Delete => "-",
                    similar::ChangeTag::Insert => "+",
                    similar::ChangeTag::Equal => " ",
                };
                print!("{}{}", sign, change);
            }
        }
    }

    Ok(())
}
