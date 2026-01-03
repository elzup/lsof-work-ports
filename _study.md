# Rust Study Guide: lsof-work-ports コード解説

このドキュメントでは、`lsof-work-ports` のコードを通じて、Rust の初心者から上級者まで段階的に学べる内容を解説します。

---

## 初心者編

### 1. 基本的な構造体（Struct）

```rust
#[derive(Debug, Clone)]
struct PortInfo {
    port: u16,
    process: String,
    pid: String,
    command: String,
    start_time: String,
}
```

**学べること:**
- `struct` でデータをまとめる
- `u16` は 0〜65535 の整数型（ポート番号に最適）
- `String` は所有権を持つ文字列型
- `#[derive(Debug, Clone)]` で自動的にデバッグ出力とクローン機能を追加

### 2. Option型で「値があるかもしれない」を表現

```rust
struct Cli {
    port: Option<u16>,      // --port が指定されていない場合は None
    process: Option<String>, // --process が指定されていない場合は None
    all: bool,               // フラグは常に true/false
}
```

**学べること:**
- `Option<T>` は値があるかないかを安全に扱う
- `Some(値)` または `None` のどちらか
- null ポインタエラーを防ぐ Rust の仕組み

### 3. Result型でエラー処理

```rust
fn get_port_info() -> Result<Vec<PortInfo>> {
    let output = Command::new("lsof")
        .args(["-i", "-P", "-n"])
        .output()
        .context("Failed to execute lsof command")?;

    // ... 処理 ...

    Ok(port_infos)
}
```

**学べること:**
- `Result<T, E>` は成功（`Ok`）か失敗（`Err`）を返す
- `?` 演算子でエラーを呼び出し元に伝播
- `anyhow` クレートで簡単なエラー処理

### 4. ベクター（Vec）とイテレータ

```rust
let processes: Vec<String> = infos.iter().map(|i| i.process.clone()).collect();
```

**学べること:**
- `Vec<T>` は可変長配列
- `.iter()` でイテレータを作成
- `.map()` で各要素を変換
- `.collect()` でイテレータを Vec に戻す

---

## 中級者編

### 1. クロージャとフィルタリング

```rust
let filtered = port_infos
    .into_iter()
    .filter(|info| {
        // Port filter
        if let Some(port) = port_filter {
            if info.port != port {
                return false;
            }
        }

        // Process name filter
        if let Some(process) = process_filter {
            if !info.process.to_lowercase().contains(&process.to_lowercase()) {
                return false;
            }
        }

        // Monitored ports check
        if !all {
            config.is_monitored(info.port)
        } else {
            true
        }
    })
    .collect();
```

**学べること:**
- `|info| { ... }` はクロージャ（無名関数）
- `if let Some(x) = option` で Option の中身を取り出す
- `.filter()` で条件に合う要素だけを残す
- `into_iter()` は所有権を移動するイテレータ

### 2. HashMap を使ったグルーピング

```rust
fn group_by_port(port_infos: Vec<PortInfo>) -> Vec<GroupedPortInfo> {
    use std::collections::HashMap;

    let mut grouped: HashMap<u16, Vec<PortInfo>> = HashMap::new();
    for info in port_infos {
        grouped.entry(info.port).or_default().push(info);
    }

    grouped
        .into_iter()
        .map(|(port, infos)| {
            // ... グループ化処理 ...
        })
        .collect()
}
```

**学べること:**
- `HashMap<K, V>` でキーと値のペアを管理
- `.entry(key).or_default()` で存在しなければデフォルト値を挿入
- `into_iter()` で HashMap の所有権を消費
- タプル `(port, infos)` でキーと値を同時に取り出す

### 3. パターンマッチングと文字列処理

```rust
fn matches(&self, target: u16) -> bool {
    for part in self.ports.split(',') {
        let part = part.trim();
        if let Some((start_str, end_str)) = part.split_once('-') {
            // Range: "3000-3100"
            if let (Ok(start), Ok(end)) =
                (start_str.trim().parse::<u16>(), end_str.trim().parse::<u16>())
            {
                if target >= start && target <= end {
                    return true;
                }
            }
        } else if let Ok(single_port) = part.parse::<u16>() {
            // Single port: "3000"
            if target == single_port {
                return true;
            }
        }
    }
    false
}
```

**学べること:**
- `.split()` で文字列を分割
- `.split_once()` で最初の区切り文字で2つに分割
- `.parse::<T>()` で文字列を数値に変換（失敗する可能性がある）
- ネストした `if let` でパターンマッチング

### 4. Serde を使ったシリアライズ/デシリアライズ

```rust
#[derive(Debug, Serialize, Deserialize)]
struct Config {
    #[serde(default)]
    ports: Vec<PortEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PortEntry {
    ports: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}
```

**学べること:**
- `#[derive(Serialize, Deserialize)]` で自動的に変換機能を追加
- `#[serde(default)]` でフィールドが存在しない場合のデフォルト値
- `#[serde(skip_serializing_if = "...")]` で条件付きで省略
- TOML ファイルと Rust 構造体の相互変換

---

## 上級者編

### 1. HashSet を使った重複排除（順序を保持）

```rust
let pids: Vec<String> = {
    let mut seen = HashSet::new();
    infos
        .iter()
        .filter_map(|i| {
            if seen.insert(i.pid.clone()) {
                Some(i.pid.clone())
            } else {
                None
            }
        })
        .collect()
};
```

**学べること:**
- `HashSet::insert()` は要素が新規なら `true`、既存なら `false` を返す
- `.filter_map()` でフィルタリングと変換を同時に行う
- ブロック `{ }` で一時的なスコープを作成
- Vec の順序を保ちながら重複を除去するテクニック

### 2. スコープとシャドーイング

```rust
let (mut monitored, mut non_monitored): (Vec<_>, Vec<_>) =
    grouped.into_iter().partition(|info| config.is_monitored(info.port));

// Apply sorting
if cli.sort_recent {
    monitored.sort_by(|a, b| b.start_time.cmp(&a.start_time));
    non_monitored.sort_by(|a, b| b.start_time.cmp(&a.start_time));
} else {
    monitored.sort_by_key(|info| info.port);
    non_monitored.sort_by_key(|info| info.port);
}

// Apply limit
let limit = if cli.limit > 0 { cli.limit } else { usize::MAX };
let monitored: Vec<_> = monitored.into_iter().take(limit).collect();
let non_monitored: Vec<_> = non_monitored.into_iter().take(limit).collect();
```

**学べること:**
- `.partition()` で条件に基づいて2つに分割
- 型推論 `Vec<_>` でコンパイラに型を推論させる
- シャドーイング: 同じ変数名で再定義（ミュータブル→イミュータブル）
- `into_iter().take(n)` で最初の n 個だけ取得

### 3. カスタムソート

```rust
// 文字列で比較（逆順）
monitored.sort_by(|a, b| b.start_time.cmp(&a.start_time));

// キーを使ってソート（昇順）
monitored.sort_by_key(|info| info.port);
```

**学べること:**
- `.sort_by()` でカスタム比較関数を使う
- `.cmp()` で比較結果（`Ordering`）を返す
- `.sort_by_key()` でキー抽出関数を使う（より簡潔）
- `b.cmp(&a)` で逆順ソート

### 4. UTF-8 文字列の正しい扱い

```rust
let prefix_len = 6 + 1 + PROCESS_WIDTH + 1 + pid_display.chars().count() + 2;
let max_command_len = term_width.saturating_sub(prefix_len);
let display_command = if info.command.chars().count() > max_command_len {
    info.command.chars().take(max_command_len).collect::<String>()
} else {
    info.command.clone()
};
```

**学べること:**
- `.chars()` で Unicode スカラー値のイテレータを取得
- `.chars().count()` で文字数を数える（`.len()` はバイト数）
- `.chars().take(n)` で最初の n 文字を取得
- `.saturating_sub()` で 0 未満にならない減算

### 5. include_str! マクロと遅延評価

```rust
impl Default for Config {
    fn default() -> Self {
        const DEFAULT_CONFIG: &str = include_str!("../default-config.toml");
        toml::from_str(DEFAULT_CONFIG).unwrap_or_else(|_| Self { ports: Vec::new() })
    }
}
```

**学べること:**
- `include_str!()` でコンパイル時にファイルを埋め込み
- `const` でコンパイル時定数
- `.unwrap_or_else(|_| ...)` で遅延評価のデフォルト値
- `Self` で現在の型を参照

### 6. clap の derive マクロ

```rust
#[derive(Parser)]
#[command(name = "lsof-work-ports")]
#[command(about = "Manage ports occupied by processes", long_about = None)]
struct Cli {
    #[arg(short, long)]
    port: Option<u16>,

    #[arg(short = 'n', long)]
    process: Option<String>,

    #[arg(short = 'l', long, default_value = "0")]
    limit: usize,
}
```

**学べること:**
- `#[derive(Parser)]` でコマンドライン引数パーサを自動生成
- `#[arg(short, long)]` で `-p` と `--port` の両方に対応
- `#[arg(short = 'n')]` で短縮形をカスタマイズ
- `default_value` で引数が省略された場合の値

### 7. エラーハンドリングのベストプラクティス

```rust
fn load() -> Result<Self> {
    let config_path = Self::config_path()?;
    if !config_path.exists() {
        return Ok(Self::default());
    }

    let content = std::fs::read_to_string(&config_path)
        .context("Failed to read config file")?;
    toml::from_str(&content)
        .context("Failed to parse config file")
}
```

**学べること:**
- `.context()` でエラーに追加情報を付与
- 早期リターンで正常系をネストさせない
- `Result` チェーンで簡潔なエラー処理
- `anyhow::Result` で柔軟なエラー型

---

## エキスパート編

### 1. ゼロコストアブストラクション

このコードは以下のような高レベル機能を使いながら、C言語と同等のパフォーマンスを実現しています:

```rust
stdout
    .lines()
    .skip(1)
    .filter_map(|line| {
        // ... 複雑な処理 ...
    })
    .collect()
```

**ポイント:**
- イテレータは遅延評価される
- コンパイル時に最適化され、ループと同等のコードが生成される
- 抽象化のコストがゼロ

### 2. 所有権システムの活用

```rust
// into_iter() で所有権を移動
let filtered = port_infos.into_iter().filter(...).collect();

// iter() で借用
for info in &monitored {
    display_grouped_port_info(info);
}
```

**ポイント:**
- `into_iter()` は所有権を消費、再利用不可
- `iter()` は借用、元のデータはそのまま
- `.collect()` で新しい Vec を作成
- コンパイラが自動的にメモリ管理

### 3. トレイトの実装パターン

```rust
impl PortEntry {
    fn matches(&self, target: u16) -> bool {
        // ...
    }
}

impl Default for Config {
    fn default() -> Self {
        // ...
    }
}

impl Config {
    fn is_monitored(&self, port_num: u16) -> bool {
        self.ports.iter().any(|entry| entry.matches(port_num))
    }
}
```

**ポイント:**
- 関連関数（`fn default() -> Self`）は `::` で呼び出す
- メソッド（`fn matches(&self, ...)`）は `.` で呼び出す
- 複数の `impl` ブロックで機能を分割可能
- トレイト実装と独自メソッドを分ける

### 4. パフォーマンスを考慮した設計

```rust
// 不要なクローンを避ける
let command = infos.first().map(|i| i.command.clone()).unwrap_or_default();

// 事前にキャパシティを確保
let mut grouped: HashMap<u16, Vec<PortInfo>> = HashMap::new();

// 文字列の連結を避ける
println!(
    "{} {} {}  {}",
    port_str.cyan().bold(),
    process_display.green(),
    pid_display.bright_black(),
    display_command.bright_black()
);
```

**ポイント:**
- `.clone()` は必要な場所でのみ使う
- `HashMap` のキャパシティは自動的に拡張される
- `format!` より `println!` の方が効率的
- 文字列操作は UTF-8 を意識

---

## まとめ

このコードから学べる Rust のポイント:

### 初心者向け
- 構造体、Option、Result の基本
- イテレータとベクター

### 中級者向け
- クロージャとフィルタリング
- HashMap、パターンマッチング
- Serde によるシリアライズ

### 上級者向け
- 重複排除のテクニック
- UTF-8 文字列の正しい扱い
- カスタムソート
- マクロの活用

### エキスパート向け
- ゼロコストアブストラクション
- 所有権システムの活用
- パフォーマンスを考慮した設計

実際のコードを読みながら、段階的に理解を深めていくことで、Rust の強力な機能を効果的に使えるようになります。
