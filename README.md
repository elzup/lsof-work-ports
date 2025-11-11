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
[ports.3000]
name = "React Dev Server"
category = "Frontend"
priority = 1

[ports.8080]
name = "HTTP Server"
category = "Backend"
priority = 2

[ports.5432]
name = "PostgreSQL"
category = "Database"
priority = 3
```

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
