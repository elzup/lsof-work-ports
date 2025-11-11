# lsof-work-ports

プロセスに占有されているポートを管理するRustプログラム。どこで使われているのかをすぐに発見できます。

## 特徴

- `lsof`コマンドをラップして、プロセスが占有しているポートを見やすく表示
- 一般的な開発用ポート（3000, 8080, 5173など）をデフォルトで監視
- ポート番号やプロセス名でフィルタリング可能
- カスタム設定ファイルで監視対象ポートをカスタマイズ
- ポートのカテゴリ分け（Frontend, Backend, Database, Cache）と優先順位設定

## インストール

```bash
cargo install --path .
```

または、リリースビルド:

```bash
cargo build --release
# バイナリは target/release/lsof-work-ports に生成されます
```

## 使い方

### 基本的な使い方

デフォルトでは、設定ファイルに登録されている開発用ポートのみを表示:

```bash
lsof-work-ports
```

### すべてのポートを表示

```bash
lsof-work-ports --all
```

### 特定のポートでフィルタ

```bash
lsof-work-ports --port 3000
```

### 特定のプロセス名でフィルタ

```bash
lsof-work-ports --process node
```

### 設定ファイルの初期化

デフォルト設定で設定ファイルを生成:

```bash
lsof-work-ports init
```

設定ファイルは `~/.config/lsof-work-ports/config.toml` に作成されます。

## 設定ファイル

設定ファイル (`~/.config/lsof-work-ports/config.toml`) の例:

```toml
# Individual port definitions
[[ports]]
port = 3000
name = "My React App"
category = "Frontend"
priority = 1

[[ports]]
port = 8080
name = "HTTP Server"
category = "Backend"
priority = 2

# Port range definitions
[[port_ranges]]
start = 3000
end = 3100
name = "Frontend Dev Servers"
category = "Frontend"
priority = 1

[[port_ranges]]
start = 8000
end = 9000
name = "Backend Services"
category = "Backend"
priority = 2
```

### 設定の特徴

- **個別ポート設定**: 特定のポート番号を監視
- **ポート範囲設定**: 連続したポート範囲を一括監視（例: 3000-3100）
- **カテゴリ分け**: Frontend, Backend, Database, Cacheなど
- **優先順位**: 数値で優先度を指定（将来の機能拡張用）

設定ファイルがない場合は、デフォルト設定が使用されます。

### デフォルトで監視されるポート

- **Frontend**
  - 3000: React Dev Server
  - 3001: Next.js Dev
  - 5173: Vite Dev Server

- **Backend**
  - 4000: API Server
  - 8000: HTTP Server Alt
  - 8080: HTTP Server

- **Database**
  - 3306: MySQL
  - 5432: PostgreSQL
  - 27017: MongoDB

- **Cache**
  - 6379: Redis

## 出力例

```
┌──────┬──────────┬───────┬──────────┬──────────┐
│ PORT │ PROCESS  │ PID   │ TYPE     │ CATEGORY │
├──────┼──────────┼───────┼──────────┼──────────┤
│ 3000 │ node     │ 12345 │ TCP      │ Frontend │
│ 8080 │ python3  │ 23456 │ TCP      │ Backend  │
│ 5432 │ postgres │ 34567 │ TCP      │ Database │
└──────┴──────────┴───────┴──────────┴──────────┘

3 ポート検出
```

## 開発

```bash
# 開発ビルド
cargo build

# テスト実行
cargo test

# リリースビルド
cargo build --release
```

## ライセンス

MIT
