use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::process::Command;
use terminal_size::{Width, terminal_size};

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

    /// Show all ports (default: only dev processes)
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
    address: String,    // IP address (e.g., "127.0.0.1", "0.0.0.0", "*")
}

#[derive(Debug, Clone)]
struct GroupedPortInfo {
    port: u16,
    processes: Vec<String>,
    pids: Vec<String>,
    command: String,
    start_time: String, // Most recent start time from the group
    is_local: bool,     // Whether this is a local address (127.0.0.1, 0.0.0.0, etc.)
    dev_score: u32,     // Development process score
}

#[derive(Debug, Clone)]
struct ProcessGroup {
    process_name: String,
    port_pid_pairs: Vec<(u16, String)>, // (port, pid) pairs
    command: String,
    start_time: String,
    is_local: bool, // Whether this group contains local addresses
}

// ============================================================================
// Development Process Detection
// ============================================================================
//
// Keywords and process names used to identify development servers.
//

/// Default development runtime processes
const DEFAULT_DEV_PROCESSES: &[&str] = &[
    // JavaScript/TypeScript
    "node", "deno", "bun", "ts-node", "tsx", "npx",
    // Python
    "python", "python3", "uvicorn", "gunicorn", "uwsgi", "hypercorn",
    // Ruby
    "ruby", "puma", "unicorn", "thin", "passenger",
    // PHP
    "php", "php-fpm",
    // Java/JVM
    "java", "kotlin", "scala", "gradle", "mvn",
    // Go
    "go",
    // Rust
    "cargo", "rust-analyzer", "rustc",
    // .NET
    "dotnet",
    // Elixir/Erlang
    "elixir", "erl", "beam", "mix", "iex",
    // Other languages
    "lua", "luajit", "perl", "R", "Rscript", "julia",
];

/// Default development tool keywords (matched against command line)
const DEFAULT_DEV_KEYWORDS: &[&str] = &[
    // Build tools & bundlers
    "webpack", "vite", "esbuild", "rollup", "parcel", "turbopack",
    "gulp", "grunt", "snowpack", "wmr",
    // Frontend frameworks
    "next", "nuxt", "remix", "gatsby", "astro", "svelte", "angular", "vue", "react",
    "solid", "qwik", "fresh",
    // Backend frameworks (Node.js)
    "express", "nestjs", "koa", "hono", "fastify", "restify", "hapi", "polka",
    // Backend frameworks (Python)
    "django", "flask", "fastapi", "starlette", "sanic", "tornado", "aiohttp",
    // Backend frameworks (Ruby)
    "rails", "sinatra", "hanami", "roda",
    // Backend frameworks (PHP)
    "laravel", "symfony", "artisan",
    // Backend frameworks (Go)
    "gin", "echo", "fiber", "chi", "mux",
    // Backend frameworks (Rust)
    "actix", "axum", "rocket", "warp",
    // Backend frameworks (Java)
    "spring", "quarkus", "micronaut",
    // Backend frameworks (Elixir)
    "phoenix", "plug",
    // Package managers & runners
    "npm", "yarn", "pnpm", "pip", "poetry", "pdm", "uv",
    "bundle", "gem", "composer", "mix",
    // Common dev commands
    "dev", "serve", "start", "watch", "hot-reload", "livereload", "run",
    // Testing
    "jest", "vitest", "playwright", "cypress", "mocha", "pytest", "rspec",
    // Database tools
    "prisma", "drizzle", "typeorm", "sequelize", "knex",
    // Other dev tools
    "storybook", "docusaurus", "vuepress", "vitepress",
];

/// Default processes to exclude from dev detection
const DEFAULT_EXCLUDE_PROCESSES: &[&str] = &[
    // IDE/Editor helpers (they listen on ports but aren't dev servers)
    "Code Helper",
    "Electron Helper",
    "Chrome Helper",
    "Chromium Helper",
    // System processes
    "Dropbox",
    "OneDrive",
    "Creative Cloud",
];

/// Minimum score threshold to be considered a dev process
const DEV_SCORE_THRESHOLD: u32 = 30;

/// Calculate development score for a process
fn calc_dev_score(
    process: &str,
    command: &str,
    port: u16,
    address: &str,
    dev_processes: &[String],
    dev_keywords: &[String],
    exclude_processes: &[String],
) -> u32 {
    let process_lower = process.to_lowercase();
    let command_lower = command.to_lowercase();

    // Check exclusion list first (matches process name or command)
    if exclude_processes.iter().any(|e| {
        let e_lower = e.to_lowercase();
        process_lower.contains(&e_lower) || command_lower.contains(&e_lower)
    }) {
        return 0;
    }

    let mut score = 0;

    // Process name match (+30)
    if dev_processes
        .iter()
        .any(|p| process_lower.contains(&p.to_lowercase()))
    {
        score += 30;
    }

    // Command line keyword match (+25)
    if dev_keywords
        .iter()
        .any(|k| command_lower.contains(&k.to_lowercase()))
    {
        score += 25;
    }

    // Local address (+10)
    if is_local_address(address) {
        score += 10;
    }

    // Common dev port ranges (+15)
    if matches!(
        port,
        3000..=3999 | 4000..=4999 | 5000..=5999 | 8000..=8999 | 9000..=9999
    ) {
        score += 15;
    }

    score
}

// ============================================================================
// Display Format Configuration
// ============================================================================
//
// This section contains all display formatting settings and functions.
// Modify these constants and functions to customize the output format.
//

/// Display format configuration constants
mod display_config {
    /// Width of the process name column (in characters)
    pub const PROCESS_WIDTH: usize = 20;

    /// Indicator shown for local addresses (127.0.0.1, 0.0.0.0, localhost, etc.)
    pub const LOCAL_INDICATOR_LOCAL: &str = "L ";

    /// Indicator shown for remote/non-local addresses
    pub const LOCAL_INDICATOR_REMOTE: &str = "  ";
}

/// Format a single PID:port pair
///
/// Returns: `[pid]:port` format (e.g., `[12345]:3000`)
fn format_pid(pid: &str) -> String {
    format!("[{pid}]")
}

/// Format PID with port (for process groups with multiple ports)
fn format_pid_with_port(pid: &str, port: u16) -> String {
    format!("[{pid}]:{port}")
}

/// Format multiple PIDs with optional truncation
///
/// # Arguments
/// * `pids` - List of process IDs
/// * `max_display` - Maximum number of PIDs to display (None for all)
///
/// # Returns
/// Comma-separated list of `[pid]` with truncation if needed
fn format_pid_list(pids: &[String], max_display: Option<usize>) -> String {
    match max_display {
        Some(max) if pids.len() > max => {
            let pairs: Vec<String> = pids[..max].iter().map(|pid| format_pid(pid)).collect();
            format!("{}, ... (x{})", pairs.join(", "), pids.len())
        }
        _ => {
            let pairs: Vec<String> = pids.iter().map(|pid| format_pid(pid)).collect();
            pairs.join(", ")
        }
    }
}

/// Check if an address is local (localhost, 127.0.0.1, 0.0.0.0, etc.)
fn is_local_address(address: &str) -> bool {
    matches!(
        address,
        "127.0.0.1" | "localhost" | "0.0.0.0" | "*" | "[::1]" | "[::]"
    )
}

// ============================================================================
// End Display Format Configuration
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
struct Config {
    /// Process names that indicate development (e.g., "node", "python")
    #[serde(default)]
    dev_processes: Vec<String>,

    /// Keywords in command line that indicate development (e.g., "webpack", "vite")
    #[serde(default)]
    dev_keywords: Vec<String>,

    /// Process names to exclude from dev detection (e.g., "Code Helper")
    #[serde(default)]
    exclude_processes: Vec<String>,

    /// Minimum score to be considered a dev process (default: 30)
    #[serde(default = "default_score_threshold")]
    score_threshold: u32,
}

fn default_score_threshold() -> u32 {
    DEV_SCORE_THRESHOLD
}

impl Default for Config {
    fn default() -> Self {
        const DEFAULT_CONFIG: &str = include_str!("../default-config.toml");
        toml::from_str(DEFAULT_CONFIG).unwrap_or_else(|_| Self {
            dev_processes: DEFAULT_DEV_PROCESSES.iter().map(|s| s.to_string()).collect(),
            dev_keywords: DEFAULT_DEV_KEYWORDS.iter().map(|s| s.to_string()).collect(),
            exclude_processes: DEFAULT_EXCLUDE_PROCESSES.iter().map(|s| s.to_string()).collect(),
            score_threshold: DEV_SCORE_THRESHOLD,
        })
    }
}

impl Config {
    fn load() -> Result<Self> {
        let config_path = Self::config_path()?;
        if !config_path.exists() {
            return Ok(Self::default());
        }

        let content =
            std::fs::read_to_string(&config_path).context("Failed to read config file")?;
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
            // Only include LISTEN state (skip ESTABLISHED, etc.)
            if !line.contains("(LISTEN)") {
                return None;
            }

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
                    address: extract_address(name_field),
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

fn extract_address(name_field: &str) -> String {
    // Format examples: "*:8080", "127.0.0.1:3000", "localhost:5000"
    if let Some(addr) = name_field.split(':').next() {
        addr.to_string()
    } else {
        "*".to_string()
    }
}

fn filter_port_infos(
    port_infos: Vec<PortInfo>,
    port_filter: Option<u16>,
    process_filter: Option<&str>,
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
                if !info
                    .process
                    .to_lowercase()
                    .contains(&process.to_lowercase())
                {
                    return false;
                }
            }

            true
        })
        .collect()
}

/// Returns a closure that deduplicates items by a key function
/// Usage: items.iter().filter_map(dedup_by(|item| item.key.clone())).collect()
fn dedup_by<T, F, K>(mut key_fn: F) -> impl FnMut(&T) -> Option<K>
where
    F: FnMut(&T) -> K,
    K: Eq + std::hash::Hash + Clone,
{
    use std::collections::HashSet;
    let mut seen = HashSet::new();
    move |item| {
        let key = key_fn(item);
        if seen.insert(key.clone()) {
            Some(key)
        } else {
            None
        }
    }
}

fn deduplicate_pids(infos: &[PortInfo]) -> Vec<String> {
    infos
        .iter()
        .filter_map(dedup_by(|i: &PortInfo| i.pid.clone()))
        .collect()
}

fn group_by_port(port_infos: Vec<PortInfo>, config: &Config) -> Vec<GroupedPortInfo> {
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
            let start_time = infos
                .first()
                .map(|i| i.start_time.clone())
                .unwrap_or_default();
            let first = infos.first();
            let is_local = first.map(|i| is_local_address(&i.address)).unwrap_or(false);
            let address = first.map(|i| i.address.as_str()).unwrap_or("*");
            let process = first.map(|i| i.process.as_str()).unwrap_or("");

            let dev_score = calc_dev_score(
                process,
                &command,
                port,
                address,
                &config.dev_processes,
                &config.dev_keywords,
                &config.exclude_processes,
            );

            GroupedPortInfo {
                port,
                processes,
                pids,
                command,
                start_time,
                is_local,
                dev_score,
            }
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
            let start_time = infos
                .first()
                .map(|i| i.start_time.clone())
                .unwrap_or_default();
            let is_local = infos.first().map(|i| i.is_local).unwrap_or(false);

            ProcessGroup {
                process_name,
                port_pid_pairs,
                command,
                start_time,
                is_local,
            }
        })
        .collect()
}

fn display_grouped_port_info(info: &GroupedPortInfo, show_multi_line: bool) {
    use display_config::*;

    // Get terminal width, default to 80 if unavailable
    let term_width = terminal_size()
        .map(|(Width(w), _)| w as usize)
        .unwrap_or(80);

    // Local indicator (2 chars: "L " or "  ")
    let local_indicator = if info.is_local {
        LOCAL_INDICATOR_LOCAL
    } else {
        LOCAL_INDICATOR_REMOTE
    };

    // Fixed width for port (6 chars: ":12345"), left-aligned
    let port_str = format!(":{:<5}", info.port);

    // Fixed width for process display
    let process_display = if info.processes.len() == 1 {
        format!("{:<width$}", info.processes[0], width = PROCESS_WIDTH)
    } else {
        let first_process = &info.processes[0];
        let count_str = format!("{}, ... (x{})", first_process, info.processes.len());
        format!("{:<width$}", count_str, width = PROCESS_WIDTH)
    };

    // PID display - limit to first 3 PIDs if too many
    let pid_display = if info.pids.len() <= 3 {
        format_pid_list(&info.pids, None)
    } else {
        format_pid_list(&info.pids, Some(2))
    };

    // Calculate available space for command (account for local_indicator)
    let prefix_len = 2 + 6 + 1 + PROCESS_WIDTH + 1 + pid_display.chars().count() + 2;
    let max_command_len = term_width.saturating_sub(prefix_len);
    let display_command = if info.command.chars().count() > max_command_len {
        info.command
            .chars()
            .take(max_command_len)
            .collect::<String>()
    } else {
        info.command.clone()
    };

    println!(
        "{}{} {} {}  {}",
        local_indicator,
        port_str.cyan().bold(),
        process_display.green(),
        pid_display.bright_black(),
        display_command.bright_black()
    );

    // Multi-line display for processes with multiple PIDs
    if show_multi_line && info.pids.len() > 1 {
        // Create PID list for all PIDs
        let pid_list: Vec<String> = info.pids.iter().map(|pid| format_pid(pid)).collect();

        // Display all PIDs on second line
        println!("{}", pid_list.join(", ").bright_black());
    }
}

fn display_process_group(group: &ProcessGroup) {
    use display_config::*;

    // Get terminal width, default to 80 if unavailable
    let term_width = terminal_size()
        .map(|(Width(w), _)| w as usize)
        .unwrap_or(80);

    // Local indicator (2 chars: "L " or "  ")
    let local_indicator = if group.is_local {
        LOCAL_INDICATOR_LOCAL
    } else {
        LOCAL_INDICATOR_REMOTE
    };

    // Fixed width for process display
    let process_display = format!("{:<width$}", group.process_name, width = PROCESS_WIDTH);

    // Count display
    let count_display = format!("(x{} ports)", group.port_pid_pairs.len());

    // Calculate available space for command (account for local_indicator)
    let prefix_len = 2 + PROCESS_WIDTH + 1 + count_display.chars().count() + 2;
    let max_command_len = term_width.saturating_sub(prefix_len);
    let display_command = if group.command.chars().count() > max_command_len {
        group
            .command
            .chars()
            .take(max_command_len)
            .collect::<String>()
    } else {
        group.command.clone()
    };

    println!(
        "{}{} {}  {}",
        local_indicator,
        process_display.green().bold(),
        count_display.bright_black(),
        display_command.bright_black()
    );

    // Display all port:pid pairs on second line in [pid]:port format
    let port_pid_strs: Vec<String> = group
        .port_pid_pairs
        .iter()
        .map(|(port, pid)| format_pid_with_port(pid, *port))
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

    let filtered = filter_port_infos(port_infos, cli.port, cli.process.as_deref());

    if filtered.is_empty() {
        println!("{}", "No ports found".yellow());
        return Ok(());
    }

    let grouped = group_by_port(filtered, &config);

    // Separate into categories: dev (score >= threshold), non-dev
    let (dev_processes, non_dev): (Vec<_>, Vec<_>) = grouped
        .into_iter()
        .partition(|info| info.dev_score >= config.score_threshold);

    // If --all is not set, only show dev processes
    let non_dev = if cli.all { non_dev } else { vec![] };

    // Group non-dev by process name to detect multi-port processes
    let (mut others, mut multis, process_group_items): (Vec<_>, Vec<_>, Vec<_>) = {
        use std::collections::HashMap;
        let mut by_process: HashMap<String, Vec<GroupedPortInfo>> = HashMap::new();

        for item in non_dev {
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

    let mut dev_processes = dev_processes;

    // Group process_group_items by process name
    let mut process_groups = group_by_process(process_group_items);

    // Apply sorting
    if cli.sort_recent {
        // Sort by start time (most recent first)
        dev_processes.sort_by(|a, b| b.start_time.cmp(&a.start_time));
        others.sort_by(|a, b| b.start_time.cmp(&a.start_time));
        multis.sort_by(|a, b| b.start_time.cmp(&a.start_time));
        process_groups.sort_by(|a, b| b.start_time.cmp(&a.start_time));
    } else {
        // Default: sort by dev_score (descending), then port number
        dev_processes.sort_by(|a, b| b.dev_score.cmp(&a.dev_score).then(a.port.cmp(&b.port)));
        others.sort_by_key(|info| info.port);
        multis.sort_by_key(|info| info.port);
        process_groups.sort_by_key(|g| g.process_name.clone());
    }

    // Apply limit
    let limit = if cli.limit > 0 { cli.limit } else { usize::MAX };
    let dev_processes: Vec<_> = dev_processes.into_iter().take(limit).collect();
    let others: Vec<_> = others.into_iter().take(limit).collect();
    let multis: Vec<_> = multis.into_iter().take(limit).collect();
    let process_groups: Vec<_> = process_groups.into_iter().take(limit).collect();

    let total_count = dev_processes.len() + others.len() + multis.len() + process_groups.len();
    println!("\n{} port(s) detected:\n", total_count);

    // Display dev processes first
    if !dev_processes.is_empty() {
        println!("{}", "dev".bright_blue().bold());
        for info in &dev_processes {
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
