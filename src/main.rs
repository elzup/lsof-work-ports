use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::process::Command;
use terminal_size::{terminal_size, Width};

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

#[derive(Debug, Clone)]
struct PortInfo {
    port: u16,
    process: String,
    pid: String,
    command: String,
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

            let process = parts[0];
            let pid = parts[1];
            let name_field = parts[8];

            // Get command line
            let command = get_process_command(pid).unwrap_or_else(|_| process.to_string());

            extract_port(name_field).and_then(|port_str| {
                port_str.parse::<u16>().ok().map(|port| PortInfo {
                    port,
                    process: process.into(),
                    pid: pid.into(),
                    command,
                })
            })
        })
        .collect())
}

fn get_process_command(pid: &str) -> Result<String> {
    let output = Command::new("ps")
        .args(["-p", pid, "-o", "command="])
        .output()
        .context("Failed to execute ps command")?;

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
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

            // If not showing all, only show monitored ports
            if !all {
                config.is_monitored(info.port)
            } else {
                true
            }
        })
        .collect()
}

fn display_port_info(info: &PortInfo) {
    // Get terminal width, default to 80 if unavailable
    let term_width = terminal_size().map(|(Width(w), _)| w as usize).unwrap_or(80);

    // Fixed width for port (6 chars: ":12345"), left-aligned
    let port_str = format!(":{:<5}", info.port);
    // Fixed width for process name (15 chars)
    let process_str = format!("{:15}", info.process);
    // PID with brackets
    let pid_str = format!("(PID: {})", info.pid);

    // Calculate available space for command
    let prefix_len = 6 + 1 + 15 + 1 + pid_str.chars().count() + 2; // port + space + process + space + pid + "  "
    let max_command_len = term_width.saturating_sub(prefix_len);
    let display_command = if info.command.chars().count() > max_command_len {
        info.command.chars().take(max_command_len).collect::<String>()
    } else {
        info.command.clone()
    };

    println!(
        "{} {} {}  {}",
        port_str.cyan().bold(),
        process_str.green(),
        pid_str.bright_black(),
        display_command.bright_black()
    );
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

    println!("\n{} port(s) detected:\n", filtered.len());
    for info in &filtered {
        display_port_info(info);
    }

    Ok(())
}
