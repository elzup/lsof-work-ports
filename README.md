# lsof-work-ports

A Rust program to manage and monitor ports occupied by processes. Quickly discover which processes are using which ports.

## Features

- Wraps `lsof` command to display port usage in a clean format
- Monitors common development ports (3000, 8080, 5173, etc.) by default
- Filter by port number or process name
- Customizable port monitoring via config file
- Supports flexible port specifications: single ports, ranges, comma-separated, and mixed formats
- Groups multiple processes on the same port
- Sorting options: by port number or recent activity
- Configurable display limit

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

By default, shows only monitored ports from config:

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
# Single port
[[ports]]
ports = "3000"
name = "My React App"

# Port range
[[ports]]
ports = "3000-3100"
name = "Frontend Dev Servers"

# Comma-separated ports
[[ports]]
ports = "6000,6001,6002"
name = "Cache Servers"

# Mixed format
[[ports]]
ports = "7000-7010,7777,8888,9000-9010"
name = "Mixed Ports"

# Name is optional
[[ports]]
ports = "4000"
```

### Port specification formats

- **Single port**: `"3000"`
- **Range**: `"3000-3100"`
- **Comma-separated**: `"6000,6001,6002"`
- **Mixed**: `"7000-7010,7777,8888,9000-9010"`

If no config file exists, default settings will be used.

### Default monitored ports

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

## Output example

```
5 port(s) detected:

:3000  ruby                 (PID: 7894)   puma 3.12.6 (tcp://0.0.0.0:3000)
:5000  ControlCe, ... (x2)  (PIDs: 94528) /System/Library/CoreServices/ControlCenter
:5353  Opera, ... (x24)     (PIDs: 28951, 98294, ... x2) /Applications/Opera.app

:3306  MySQLWork            (PID: 43260)  /Applications/MySQLWorkbench.app
:6379  com.docke            (PID: 9340)   /Applications/Docker.app/Contents/MacOS
```

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
