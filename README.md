# lsof-work-ports

A Rust CLI tool to discover and monitor development server ports. Automatically detects dev processes based on process names and command-line keywords.

## Features

- Wraps `lsof` command to display port usage in a clean format
- **Auto-detects development processes** using a scoring system:
  - Process name matching (node, python, ruby, etc.)
  - Command-line keyword matching (webpack, vite, next, etc.)
  - Local address detection
  - Common dev port ranges (3000-9999)
- Filter by port number or process name
- Customizable detection via config file
- Groups multiple processes on the same port
- Sorting options: by score, port number, or recent activity

## Installation

```bash
cargo install --path .
```

Or build from source:

```bash
cargo build --release
# Binary will be at target/release/lsof-work-ports
```

## Usage

### Basic usage

By default, shows only detected dev processes:

```bash
lsof-work-ports
```

### Show all ports

```bash
lsof-work-ports --all
```

### Filter by specific port

```bash
lsof-work-ports --port 3000
```

### Filter by process name

```bash
lsof-work-ports --process node
```

### Limit output

```bash
# Show first 10 ports
lsof-work-ports --limit 10

# Show 10 most recently started processes
lsof-work-ports --all --sort-recent --limit 10
```

### Initialize config file

Generate config file with defaults:

```bash
lsof-work-ports init
```

Config file will be created at `~/.config/lsof-work-ports/config.toml`.

## Configuration

Example config file (`~/.config/lsof-work-ports/config.toml`):

```toml
# Process names that indicate development servers
dev_processes = [
    "node",
    "python",
    "ruby",
    "php",
    "go",
    "cargo",
]

# Keywords in command line that indicate development
dev_keywords = [
    "webpack",
    "vite",
    "next",
    "dev",
    "serve",
    "start",
]

# Minimum score to be considered a dev process (default: 30)
score_threshold = 30
```

### Scoring System

Each process is scored based on multiple factors:

| Factor | Score |
|--------|-------|
| Process name match (node, python, etc.) | +30 |
| Command keyword match (webpack, vite, etc.) | +25 |
| Local address (127.0.0.1, 0.0.0.0) | +10 |
| Dev port range (3000-9999) | +15 |

Processes with score >= `score_threshold` are shown in the `dev` section.

## Output example

```
20 port(s) detected:

dev
L :62267 php                            [91310]:62267  PHP Language Server
  :53852 node                           [18186]:53852  next-server (v15.5.7)
L :3000  node                           [12345]:3000   vite dev server

others
L :3306  MySQLWork                      [8207]:3306  /Applications/MySQLWorkbench
  :80    nginx                          [1234]:80    nginx: master process

process_groups
L Dropbox                        (x2 ports)  /Applications/Dropbox.app
[84278]:17600, [84278]:17603
```

- `L` prefix indicates local address (127.0.0.1, 0.0.0.0, etc.)
- `dev` section: Detected development processes (score >= threshold)
- `others` section: Non-dev processes (shown with `--all`)
- `process_groups` section: Processes using multiple ports

## Development

```bash
# Development build
cargo build

# Run tests
cargo test

# Release build
cargo build --release
```

## License

MIT
