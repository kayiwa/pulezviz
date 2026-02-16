# PulEzViz 📊

**EZproxy Log Analytics Dashboard** - A high-performance, real-time visualization tool for analyzing EZproxy server logs.

Built with Rust, DuckDB, and Chart.js to handle millions of log entries with ease.

## Features

**9 Interactive Visualizations**
- Request volume over time (hourly aggregation)
- Bandwidth usage tracking (MB/hour)
- Top accessed hosts/domains
- HTTP status code distribution
- Geographic access patterns by country
- Usage heatmap by day of week
- Error analysis with 4xx/5xx breakdown
- Browser/user agent distribution
- Most accessed paths with average file sizes

**High Performance**
- Handles 1M+ log entries efficiently
- Fast DuckDB columnar database backend
- Batch import with progress tracking
- Real-time dashboard with no page reloads

**Modern UI**
- Responsive grid layout
- Interactive Chart.js visualizations
- Hover effects and smooth transitions
- Gradient design

## Prerequisites

- Rust (1.70 or later)
- Cargo (comes with Rust)

Install Rust from [rustup.rs](https://rustup.rs/):
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

## Installation

```bash
# Clone the repository
git clone https://github.com/pulibrary/pulezviz.git
cd pulezviz

# Build the project
cargo build --release
```

The compiled binary will be at `target/release/pulezviz`.

## Quick Start

### 1. Import Your Logs

Import a single log file:
```bash
cargo run --release -- import ezproxy.log --db analytics.duckdb
```

Import multiple log files:
```bash
# Using the batch import script
./import_all.sh analytics.duckdb /path/to/logs/

# Or manually
for log in *.log; do
    cargo run --release -- import "$log" --db analytics.duckdb
done
```

### 2. Start the Dashboard

```bash
cargo run --release -- serve --db analytics.duckdb --bind 127.0.0.1:8080
```

### 3. View Your Analytics

Open your browser to: **http://localhost:8080**

## Usage

### Command Line Interface

```bash
pulezviz <COMMAND> [OPTIONS]

Commands:
  import    Import a log file into DuckDB
  serve     Run a local dashboard server
  help      Print this message or the help of the given subcommand(s)
```

#### Import Command

```bash
pulezviz import <LOG_PATH> [OPTIONS]

Arguments:
  <LOG_PATH>    Path to EZproxy log file

Options:
  --db <DB>     DuckDB database file [default: ezvis.duckdb]
  -h, --help    Print help
```

**Example:**
```bash
# Import with default database
cargo run --release -- import ezproxy20260215.log

# Import with custom database
cargo run --release -- import ezproxy20260215.log --db my_analytics.duckdb
```

#### Serve Command

```bash
pulezviz serve [OPTIONS]

Options:
  --db <DB>       DuckDB database file [default: ezvis.duckdb]
  --bind <BIND>   Bind address [default: 127.0.0.1:8080]
  -h, --help      Print help
```

**Example:**
```bash
# Serve on default port
cargo run --release -- serve --db analytics.duckdb

# Serve on custom port
cargo run --release -- serve --db analytics.duckdb --bind 0.0.0.0:3000
```

## Log Format

PulEzViz expects standard EZproxy log format:
```
<IP> <identd> <user/session> [<timestamp>] "<method> <url> <http_version>" <status> <bytes> "<country>" "<user_agent>"
```

**Example:**
```
10.50.3.252 - sCyGAlJG8RoCLDry3ziUL4lk7NXPtMH [15/Feb/2026:00:00:04 +0000] "GET https://www.jstor.org:443/stable/12345 HTTP/1.1" 200 251752 "US" "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36"
```

## Database Schema

The tool creates a `requests` table with the following schema:

| Column          | Type         | Description                    |
|-----------------|--------------|--------------------------------|
| ts              | TIMESTAMPTZ  | Request timestamp              |
| remote_addr     | TEXT         | Client IP address              |
| identd          | TEXT         | Ident string (usually -)       |
| user_or_session | TEXT         | Username or session ID         |
| method          | TEXT         | HTTP method (GET, POST, etc.)  |
| url             | TEXT         | Full URL                       |
| scheme          | TEXT         | Protocol (http/https)          |
| host            | TEXT         | Hostname                       |
| port            | INTEGER      | Port number                    |
| path            | TEXT         | URL path                       |
| query           | TEXT         | Query string                   |
| http_version    | TEXT         | HTTP version                   |
| status          | INTEGER      | HTTP status code               |
| bytes           | BIGINT       | Response size in bytes         |
| country         | TEXT         | Country code                   |
| user_agent      | TEXT         | Browser/client user agent      |
| raw             | TEXT         | Original log line              |

Indexes are automatically created on `ts`, `host`, `status`, and `country` for optimal query performance.

## Batch Import Script

For importing multiple log files efficiently:

```bash
#!/bin/bash
# import_all.sh

DB_FILE="${1:-ezvis.duckdb}"
LOG_DIR="${2:-.}"

echo "Importing logs from $LOG_DIR into $DB_FILE"
echo "=========================================="

count=0
for logfile in "$LOG_DIR"/*.log; do
    if [ -f "$logfile" ]; then
        echo "[$((++count))] Importing: $logfile"
        cargo run --release -- import "$logfile" --db "$DB_FILE"
        if [ $? -ne 0 ]; then
            echo "Failed to import $logfile"
        else
            echo "Successfully imported $logfile"
        fi
    fi
done

echo ""
echo "Import complete! Total files processed: $count"
echo "Starting dashboard server..."
cargo run --release -- serve --db "$DB_FILE"
```

Make it executable and run:
```bash
chmod +x import_all.sh
./import_all.sh analytics.duckdb /path/to/logs/
```

## Performance

**Import Performance:**
- ~100,000 rows/second on modern hardware
- Progress reporting every 10,000 entries
- Memory efficient batch processing
- Handles files with millions of entries

**Dashboard Performance:**
- Instant page loads
- Sub-second query responses
- Hourly aggregation for time series (reduces data points)
- Optimized SQL with proper indexes

**Tested with:**
- 1.07 million log entries
- Multiple days of data
- Concurrent dashboard access

## API Endpoints

The dashboard exposes these REST API endpoints:

| Endpoint                    | Description                          |
|-----------------------------|--------------------------------------|
| `/`                         | Main dashboard HTML                  |
| `/api/requests_over_time`   | Time series data (hourly)            |
| `/api/top_hosts`            | Top 15 hosts by request count        |
| `/api/status_codes`         | HTTP status code distribution        |
| `/api/top_countries`        | Top 20 countries by request count    |
| `/api/bandwidth_over_time`  | Bandwidth usage (MB/hour)            |
| `/api/hourly_heatmap`       | Hour × Day usage matrix              |
| `/api/error_analysis`       | Top 10 hosts with errors (4xx/5xx)   |
| `/api/user_agents`          | Browser distribution                 |
| `/api/top_paths`            | Top 15 paths with avg file size     |

All endpoints support optional `?start=<timestamp>&end=<timestamp>` parameters for filtering.

**Example:**
```bash
curl http://localhost:8080/api/top_hosts | jq
curl http://localhost:8080/api/requests_over_time?start=2026-02-15T00:00:00Z | jq
```

## Architecture

```
┌─────────────────┐
│  EZproxy Logs   │
│   (.log files)  │
└────────┬────────┘
         │
         │ parse & import
         ▼
┌─────────────────┐
│    DuckDB       │
│  (columnar DB)  │
└────────┬────────┘
         │
         │ SQL queries
         ▼
┌─────────────────┐
│  Axum Web       │
│  Server (Rust)  │
└────────┬────────┘
         │
         │ JSON API
         ▼
┌─────────────────┐
│   Dashboard     │
│ (HTML + Chart.js)│
└─────────────────┘
```

## Troubleshooting

### Import Issues

**Problem:** "line did not match expected format"
```bash
# Check log format
head -n 5 your_log.log

# The tool expects standard EZproxy format
# If format differs, you may need to adjust the regex in src/parser.rs
```

**Problem:** Transaction errors during import
```bash
# The current version uses DuckDB's appender API which doesn't use transactions
# If you see transaction errors, make sure you're on the latest version
```

### Dashboard Issues

**Problem:** "Requests Over Time" shows no data
```bash
# Check if data was imported
duckdb ezvis.duckdb -c "SELECT COUNT(*) FROM requests;"

# Check the time range
duckdb ezvis.duckdb -c "SELECT MIN(ts), MAX(ts) FROM requests;"
```

**Problem:** Port already in use
```bash
# Use a different port
cargo run --release -- serve --db ezvis.duckdb --bind 127.0.0.1:8081
```

**Problem:** CORS issues
```bash
# The server allows all origins by default
# If you need to restrict this, modify the CORS configuration in src/web.rs
```

## Development

### Project Structure

```
pulezviz/
├── src/
│   ├── main.rs      # CLI and main entry point
│   ├── db.rs        # Database operations and schema
│   ├── parser.rs    # Log file parsing logic
│   └── web.rs       # Web server and dashboard
├── Cargo.toml       # Dependencies and metadata
├── import_all.sh    # Batch import script
└── README.md        # This file
```

### Running Tests

```bash
# Run unit tests
cargo test

# Run with verbose output
cargo test -- --nocapture
```

### Building for Production

```bash
# Build optimized release binary
cargo build --release

# The binary will be at target/release/pulezviz
# Copy it to your deployment location
cp target/release/pulezviz /usr/local/bin/
```

## Contributing

Contributions are welcome! Areas for improvement:

- [ ] Add date range picker in UI
- [ ] Export charts as PNG/PDF
- [ ] Real-time log streaming
- [ ] Anomaly detection algorithms
- [ ] Geographic map visualization
- [ ] Custom query builder
- [ ] Alert/notification system
- [ ] Multi-database support

## License

MIT License - see LICENSE file for details

## Acknowledgments

- Built with [Rust](https://www.rust-lang.org/)
- Database: [DuckDB](https://duckdb.org/)
- Web Framework: [Axum](https://github.com/tokio-rs/axum)
- Charts: [Chart.js](https://www.chartjs.org/)
- HTTP Client: [tokio](https://tokio.rs/)

## Support

For issues, questions, or suggestions:
- Open an issue on GitHub
