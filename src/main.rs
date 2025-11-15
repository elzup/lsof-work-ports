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

    /// Number of ports to display (default: all)
    #[arg(short = 'l', long, default_value = "0")]
    limit: usize,

    /// Sort by port number (ascending)
    #[arg(long)]
    sort_port: bool,

    /// Sort by recent activity (most recent first)
    #[arg(long)]
    sort_recent: bool,
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
    start_time: String, // Process start time from ps
}

#[derive(Debug, Clone)]
struct GroupedPortInfo {
    port: u16,
    processes: Vec<String>,
    pids: Vec<String>,
    command: String,
    start_time: String, // Most recent start time from the group
}

#[derive(Debug, Clone)]
struct ProcessGroup {
    process_name: String,
    port_pid_pairs: Vec<(u16, String)>, // (port, pid) pairs
    command: String,
    start_time: String,
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

            // Get command line and start time
            let command = get_process_command(pid).unwrap_or_else(|_| process.to_string());
            let start_time = get_process_start_time(pid).unwrap_or_default();

            extract_port(name_field).and_then(|port_str| {
                port_str.parse::<u16>().ok().map(|port| PortInfo {
                    port,
                    process: process.into(),
                    pid: pid.into(),
                    command,
                    start_time,
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

fn get_process_start_time(pid: &str) -> Result<String> {
    let output = Command::new("ps")
        .args(["-p", pid, "-o", "lstart="])
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

fn deduplicate_pids(infos: &[PortInfo]) -> Vec<String> {
    use std::collections::HashSet;

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
}

fn group_by_port(port_infos: Vec<PortInfo>) -> Vec<GroupedPortInfo> {
    use std::collections::HashMap;

    let mut grouped: HashMap<u16, Vec<PortInfo>> = HashMap::new();
    for info in port_infos {
        grouped.entry(info.port).or_default().push(info);
    }

    grouped
        .into_iter()
        .map(|(port, infos)| {
            let processes: Vec<String> = infos.iter().map(|i| i.process.clone()).collect();
            let pids = deduplicate_pids(&infos);
            let command = infos.first().map(|i| i.command.clone()).unwrap_or_default();
            let start_time = infos.first().map(|i| i.start_time.clone()).unwrap_or_default();

            GroupedPortInfo { port, processes, pids, command, start_time }
        })
        .collect()
}

fn group_by_process(port_infos: Vec<GroupedPortInfo>) -> Vec<ProcessGroup> {
    use std::collections::HashMap;

    let mut grouped: HashMap<String, Vec<GroupedPortInfo>> = HashMap::new();
    for info in port_infos {
        // Use first process name as the group key
        let process_name = info.processes.first().cloned().unwrap_or_default();
        grouped.entry(process_name).or_default().push(info);
    }

    grouped
        .into_iter()
        .map(|(process_name, infos)| {
            let mut port_pid_pairs: Vec<(u16, String)> = Vec::new();

            // Collect all port:pid pairs
            for info in &infos {
                for pid in &info.pids {
                    port_pid_pairs.push((info.port, pid.clone()));
                }
            }

            let command = infos.first().map(|i| i.command.clone()).unwrap_or_default();
            let start_time = infos.first().map(|i| i.start_time.clone()).unwrap_or_default();

            ProcessGroup { process_name, port_pid_pairs, command, start_time }
        })
        .collect()
}

fn display_grouped_port_info(info: &GroupedPortInfo, show_multi_line: bool) {
    // Get terminal width, default to 80 if unavailable
    let term_width = terminal_size().map(|(Width(w), _)| w as usize).unwrap_or(80);

    // Fixed width for port (6 chars: ":12345"), left-aligned
    let port_str = format!(":{:<5}", info.port);

    // Fixed width for process display (30 chars - increased from 20)
    const PROCESS_WIDTH: usize = 30;
    let process_display = if info.processes.len() == 1 {
        format!("{:<width$}", info.processes[0], width = PROCESS_WIDTH)
    } else {
        let first_process = &info.processes[0];
        let count_str = format!("{}, ... (x{})", first_process, info.processes.len());
        format!("{:<width$}", count_str, width = PROCESS_WIDTH)
    };

    // PID display - limit to first 3 PIDs if too many
    let pid_display = if info.pids.len() == 1 {
        format!("(PID: {})", info.pids[0])
    } else if info.pids.len() <= 3 {
        format!("(PIDs: {})", info.pids.join(", "))
    } else {
        format!("(PIDs: {}, ... x{})", info.pids[..2].join(", "), info.pids.len())
    };

    // Calculate available space for command
    let prefix_len = 6 + 1 + PROCESS_WIDTH + 1 + pid_display.chars().count() + 2;
    let max_command_len = term_width.saturating_sub(prefix_len);
    let display_command = if info.command.chars().count() > max_command_len {
        info.command.chars().take(max_command_len).collect::<String>()
    } else {
        info.command.clone()
    };

    println!(
        "{} {} {}  {}",
        port_str.cyan().bold(),
        process_display.green(),
        pid_display.bright_black(),
        display_command.bright_black()
    );

    // Multi-line display for processes with multiple PIDs
    if show_multi_line && info.pids.len() > 1 {
        // Create port:pid pairs for all PIDs
        let port_pid_pairs: Vec<String> = info
            .pids
            .iter()
            .map(|pid| format!(":{} {}", info.port, pid))
            .collect();

        // Display all port:pid pairs on second line
        println!("{}", port_pid_pairs.join(", ").bright_black());
    }
}

fn display_process_group(group: &ProcessGroup) {
    // Get terminal width, default to 80 if unavailable
    let term_width = terminal_size().map(|(Width(w), _)| w as usize).unwrap_or(80);

    // Fixed width for process display (30 chars)
    const PROCESS_WIDTH: usize = 30;
    let process_display = format!("{:<width$}", group.process_name, width = PROCESS_WIDTH);

    // Count display
    let count_display = format!("(x{} ports)", group.port_pid_pairs.len());

    // Calculate available space for command
    let prefix_len = PROCESS_WIDTH + 1 + count_display.chars().count() + 2;
    let max_command_len = term_width.saturating_sub(prefix_len);
    let display_command = if group.command.chars().count() > max_command_len {
        group.command.chars().take(max_command_len).collect::<String>()
    } else {
        group.command.clone()
    };

    println!(
        "{} {}  {}",
        process_display.green().bold(),
        count_display.bright_black(),
        display_command.bright_black()
    );

    // Display all port:pid pairs on second line
    let port_pid_strs: Vec<String> = group
        .port_pid_pairs
        .iter()
        .map(|(port, pid)| format!(":{} {}", port, pid))
        .collect();

    println!("{}", port_pid_strs.join(", ").bright_black());
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

    let grouped = group_by_port(filtered);

    // Separate into categories: monitored, non-monitored
    let (monitored, non_monitored): (Vec<_>, Vec<_>) =
        grouped.into_iter().partition(|info| config.is_monitored(info.port));

    // Group all non-monitored by process name to detect multi-port processes
    let (mut others, mut multis, process_group_items): (Vec<_>, Vec<_>, Vec<_>) = {
        use std::collections::HashMap;
        let mut by_process: HashMap<String, Vec<GroupedPortInfo>> = HashMap::new();

        for item in non_monitored {
            let proc_name = item.processes.first().cloned().unwrap_or_default();
            by_process.entry(proc_name).or_default().push(item);
        }

        let mut others = Vec::new();
        let mut multis = Vec::new();
        let mut process_groups = Vec::new();

        for (_, items) in by_process {
            if items.len() == 1 {
                // Single port for this process
                let item = &items[0];
                if item.pids.len() == 1 {
                    // Single port, single PID -> others
                    others.push(item.clone());
                } else {
                    // Single port, multiple PIDs -> multis
                    multis.push(item.clone());
                }
            } else {
                // Multiple ports for same process -> process_groups
                process_groups.extend(items);
            }
        }

        (others, multis, process_groups)
    };

    let mut monitored = monitored;

    // Group process_group_items by process name
    let mut process_groups = group_by_process(process_group_items);

    // Apply sorting
    if cli.sort_recent {
        // Sort by start time (most recent first)
        monitored.sort_by(|a, b| b.start_time.cmp(&a.start_time));
        others.sort_by(|a, b| b.start_time.cmp(&a.start_time));
        multis.sort_by(|a, b| b.start_time.cmp(&a.start_time));
        process_groups.sort_by(|a, b| b.start_time.cmp(&a.start_time));
    } else {
        // Default: sort by port number (ascending) or process name
        monitored.sort_by_key(|info| info.port);
        others.sort_by_key(|info| info.port);
        multis.sort_by_key(|info| info.port);
        process_groups.sort_by_key(|g| g.process_name.clone());
    }

    // Apply limit
    let limit = if cli.limit > 0 { cli.limit } else { usize::MAX };
    let monitored: Vec<_> = monitored.into_iter().take(limit).collect();
    let others: Vec<_> = others.into_iter().take(limit).collect();
    let multis: Vec<_> = multis.into_iter().take(limit).collect();
    let process_groups: Vec<_> = process_groups.into_iter().take(limit).collect();

    let total_count = monitored.len() + others.len() + multis.len() + process_groups.len();
    println!("\n{} port(s) detected:\n", total_count);

    // Display monitored ports first
    if !monitored.is_empty() {
        println!("{}", "monitored".bright_blue().bold());
        for info in &monitored {
            display_grouped_port_info(info, false);
        }
        println!();
    }

    // Display single-process others
    if !others.is_empty() {
        println!("{}", "others".bright_blue().bold());
        for info in &others {
            display_grouped_port_info(info, false);
        }
        println!();
    }

    // Display multi-process same-port with 2-line format
    if !multis.is_empty() {
        println!("{}", "multis".bright_blue().bold());
        for info in &multis {
            display_grouped_port_info(info, true);
        }
        println!();
    }

    // Display process groups (same process, multiple ports)
    if !process_groups.is_empty() {
        println!("{}", "process_groups".bright_blue().bold());
        for group in &process_groups {
            display_process_group(group);
        }
    }

    Ok(())
}
