use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::process::Command;
use tabled::{Table, Tabled};

#[derive(Parser)]
#[command(name = "lsof-work-ports")]
#[command(about = "Manage ports occupied by processes", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Filter by specific port number
    #[arg(short, long)]
    port: Option<u16>,

    /// Filter by process name
    #[arg(short = 'n', long)]
    process: Option<String>,

    /// Show all ports (default: only monitored ports)
    #[arg(short, long)]
    all: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize config file
    Init,
    /// List port information
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
}

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

impl PortEntry {
    fn matches(&self, target: u16) -> bool {
        // Support formats: "3000", "3000-3100", "3000,3001,3002", "3000-3010,4000,5000-5100"
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
}

impl Default for Config {
    fn default() -> Self {
        const DEFAULT_CONFIG: &str = include_str!("../default-config.toml");
        toml::from_str(DEFAULT_CONFIG).unwrap_or_else(|_| Self { ports: Vec::new() })
    }
}

impl Config {
    fn is_monitored(&self, port_num: u16) -> bool {
        self.ports.iter().any(|entry| entry.matches(port_num))
    }

    fn load() -> Result<Self> {
        let config_path = Self::config_path()?;
        if !config_path.exists() {
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(&config_path).context("Failed to read config file")?;
        toml::from_str(&content).context("Failed to parse config file")
    }

    fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(self).context("Failed to serialize config")?;
        std::fs::write(&config_path, content).context("Failed to write config file")
    }

    fn config_path() -> Result<std::path::PathBuf> {
        let home = std::env::var("HOME").context("HOME environment variable not set")?;
        Ok(std::path::PathBuf::from(home)
            .join(".config")
            .join("lsof-work-ports")
            .join("config.toml"))
    }
}

fn get_port_info() -> Result<Vec<PortInfo>> {
    let output = Command::new("lsof")
        .args(["-i", "-P", "-n"])
        .output()
        .context("Failed to execute lsof command")?;

    anyhow::ensure!(output.status.success(), "lsof command returned an error");

    let stdout = String::from_utf8_lossy(&output.stdout);

    Ok(stdout
        .lines()
        .skip(1)
        .filter_map(|line| {
            let parts: Vec<_> = line.split_whitespace().collect();
            if parts.len() < 9 {
                return None;
            }

            extract_port(parts[8]).map(|port| PortInfo {
                port: port.into(),
                process: parts[0].into(),
                pid: parts[1].into(),
                port_type: parts[7].into(),
            })
        })
        .collect())
}

fn extract_port(name_field: &str) -> Option<&str> {
    name_field.split(':').last()
}

fn filter_port_infos(
    port_infos: Vec<PortInfo>,
    port_filter: Option<u16>,
    process_filter: Option<&str>,
    all: bool,
    config: &Config,
) -> Vec<PortInfo> {
    port_infos
        .into_iter()
        .filter(|info| {
            // Port filter
            if let Some(port) = port_filter {
                if info.port != port.to_string() {
                    return false;
                }
            }

            // Process name filter
            if let Some(process) = process_filter {
                if !info.process.to_lowercase().contains(&process.to_lowercase()) {
                    return false;
                }
            }

            // If not showing all, only show monitored ports
            if !all {
                info.port
                    .parse::<u16>()
                    .map(|port_num| config.is_monitored(port_num))
                    .unwrap_or(false)
            } else {
                true
            }
        })
        .collect()
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if let Some(Commands::Init) = &cli.command {
        let config = Config::default();
        config.save()?;
        println!("✓ Initialized config file: {:?}", Config::config_path()?);
        return Ok(());
    }

    let config = Config::load()?;
    let port_infos = get_port_info()?;

    let filtered = filter_port_infos(
        port_infos,
        cli.port,
        cli.process.as_deref(),
        cli.all,
        &config,
    );

    if filtered.is_empty() {
        println!("{}", "No ports found".yellow());
        return Ok(());
    }

    println!("{}", Table::new(&filtered));
    println!("\n{} port(s) detected", filtered.len());

    Ok(())
}
