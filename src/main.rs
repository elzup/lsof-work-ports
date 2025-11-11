use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use colored::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Command;
use tabled::{Table, Tabled};

#[derive(Parser)]
#[command(name = "lsof-work-ports")]
#[command(about = "プロセスに占有されているポートを管理するツール", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// 特定のポート番号でフィルタ
    #[arg(short, long)]
    port: Option<u16>,

    /// 特定のプロセス名でフィルタ
    #[arg(short = 'n', long)]
    process: Option<String>,

    /// すべてのポートを表示（デフォルトは監視対象のみ）
    #[arg(short, long)]
    all: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// 設定ファイルの初期化
    Init,
    /// ポート情報の一覧表示
    List,
}

#[derive(Debug, Clone, Tabled)]
struct PortInfo {
    #[tabled(rename = "PORT")]
    port: String,
    #[tabled(rename = "PROCESS")]
    process: String,
    #[tabled(rename = "PID")]
    pid: String,
    #[tabled(rename = "TYPE")]
    port_type: String,
    #[tabled(rename = "CATEGORY")]
    category: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Config {
    ports: HashMap<u16, PortConfig>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PortConfig {
    name: String,
    category: String,
    priority: u8,
}

impl Default for Config {
    fn default() -> Self {
        macro_rules! port {
            ($port:expr, $name:expr, $cat:expr, $pri:expr) => {
                (
                    $port,
                    PortConfig {
                        name: $name.to_string(),
                        category: $cat.to_string(),
                        priority: $pri,
                    },
                )
            };
        }

        let ports = HashMap::from([
            // Frontend
            port!(3000, "React Dev Server", "Frontend", 1),
            port!(3001, "Next.js Dev", "Frontend", 1),
            port!(5173, "Vite Dev Server", "Frontend", 1),
            // Backend
            port!(4000, "API Server", "Backend", 2),
            port!(8000, "HTTP Server Alt", "Backend", 2),
            port!(8080, "HTTP Server", "Backend", 2),
            // Database
            port!(3306, "MySQL", "Database", 3),
            port!(5432, "PostgreSQL", "Database", 3),
            port!(27017, "MongoDB", "Database", 3),
            // Cache
            port!(6379, "Redis", "Cache", 3),
        ]);

        Config {
            ports,
        }
    }
}

impl Config {
    fn load() -> Result<Self> {
        let config_path = Self::config_path()?;
        if !config_path.exists() {
            return Ok(Config::default());
        }

        let content =
            std::fs::read_to_string(&config_path).context("設定ファイルの読み込みに失敗")?;
        let config: Config = toml::from_str(&content).context("設定ファイルのパースに失敗")?;
        Ok(config)
    }

    fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(self).context("設定のシリアライズに失敗")?;
        std::fs::write(&config_path, content).context("設定ファイルの書き込みに失敗")?;
        Ok(())
    }

    fn config_path() -> Result<std::path::PathBuf> {
        let home = std::env::var("HOME").context("HOME環境変数が設定されていません")?;
        Ok(std::path::PathBuf::from(home)
            .join(".config")
            .join("lsof-work-ports")
            .join("config.toml"))
    }
}

fn get_port_info() -> Result<Vec<PortInfo>> {
    let output = Command::new("lsof")
        .args(&["-i", "-P", "-n"])
        .output()
        .context("lsofコマンドの実行に失敗")?;

    if !output.status.success() {
        anyhow::bail!("lsofコマンドがエラーを返しました");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut port_infos = Vec::new();

    for line in stdout.lines().skip(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 9 {
            continue;
        }

        let process = parts[0];
        let pid = parts[1];
        let port_type = parts[7];
        let name_field = parts[8];

        // ポート番号を抽出
        if let Some(port_str) = extract_port(name_field) {
            port_infos.push(PortInfo {
                port: port_str.to_string(),
                process: process.to_string(),
                pid: pid.to_string(),
                port_type: port_type.to_string(),
                category: "Unknown".to_string(),
            });
        }
    }

    Ok(port_infos)
}

fn extract_port(name_field: &str) -> Option<&str> {
    // "*:8080" や "127.0.0.1:3000" のような形式からポート番号を抽出
    name_field.split(':').last()
}

fn enrich_with_config(port_infos: &mut [PortInfo], config: &Config) {
    for info in port_infos.iter_mut() {
        if let Ok(port_num) = info.port.parse::<u16>() {
            if let Some(port_config) = config.ports.get(&port_num) {
                info.category = port_config.category.clone();
            }
        }
    }
}

fn filter_port_infos(
    port_infos: Vec<PortInfo>,
    port_filter: Option<u16>,
    process_filter: Option<String>,
    all: bool,
    config: &Config,
) -> Vec<PortInfo> {
    port_infos
        .into_iter()
        .filter(|info| {
            // ポートフィルタ
            if let Some(port) = port_filter {
                if info.port != port.to_string() {
                    return false;
                }
            }

            // プロセス名フィルタ
            if let Some(ref process) = process_filter {
                if !info.process.to_lowercase().contains(&process.to_lowercase()) {
                    return false;
                }
            }

            // 全表示でない場合は、設定にあるポートのみ
            if !all {
                if let Ok(port_num) = info.port.parse::<u16>() {
                    if !config.ports.contains_key(&port_num) {
                        return false;
                    }
                } else {
                    return false;
                }
            }

            true
        })
        .collect()
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Some(Commands::Init) => {
            let config = Config::default();
            config.save()?;
            println!("✓ 設定ファイルを初期化しました: {:?}", Config::config_path()?);
            return Ok(());
        }
        Some(Commands::List) | None => {
            // 通常の一覧表示処理
        }
    }

    let config = Config::load()?;
    let mut port_infos = get_port_info()?;

    enrich_with_config(&mut port_infos, &config);

    let filtered = filter_port_infos(port_infos, cli.port, cli.process, cli.all, &config);

    if filtered.is_empty() {
        println!("{}", "ポートが見つかりませんでした".yellow());
        return Ok(());
    }

    let table = Table::new(&filtered).to_string();
    println!("{}", table);
    println!("\n{} ポート検出", filtered.len());

    Ok(())
}
