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
    #[tabled(rename = "CATEGORY")]
    category: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Config {
    #[serde(default)]
    ports: Vec<PortEntry>,
    #[serde(default)]
    port_ranges: Vec<PortRange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PortEntry {
    port: u16,
    name: String,
    category: String,
    priority: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PortRange {
    start: u16,
    end: u16,
    name: String,
    category: String,
    priority: u8,
}

impl Default for Config {
    fn default() -> Self {
        const DEFAULT_CONFIG: &str = include_str!("../default-config.toml");
        toml::from_str(DEFAULT_CONFIG).unwrap_or_else(|_| Self {
            ports: Vec::new(),
            port_ranges: Vec::new(),
        })
    }
}

impl Config {
    fn get_port_config(&self, port_num: u16) -> Option<(&str, u8)> {
        // Check exact port match first
        if let Some(port_entry) = self.ports.iter().find(|p| p.port == port_num) {
            return Some((&port_entry.category, port_entry.priority));
        }

        // Check port ranges
        self.port_ranges
            .iter()
            .find(|range| port_num >= range.start && port_num <= range.end)
            .map(|range| (range.category.as_str(), range.priority))
    }

    fn is_monitored(&self, port_num: u16) -> bool {
        self.get_port_config(port_num).is_some()
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
                category: "Unknown".into(),
            })
        })
        .collect())
}

fn extract_port(name_field: &str) -> Option<&str> {
    name_field.split(':').last()
}

fn enrich_with_config(port_infos: &mut [PortInfo], config: &Config) {
    port_infos.iter_mut().for_each(|info| {
        if let Ok(port_num) = info.port.parse::<u16>() {
            if let Some((category, _priority)) = config.get_port_config(port_num) {
                info.category = category.to_string();
            }
        }
    });
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
    let mut port_infos = get_port_info()?;

    enrich_with_config(&mut port_infos, &config);

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
