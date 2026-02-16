#!/bin/bash
# setup.sh - Initialize PulEzViz project

set -e

echo "Setting up PulEzViz..."
echo ""

# Check for Rust installation
if ! command -v cargo &>/dev/null; then
  echo "Rust is not installed. Please install from https://rustup.rs/"
  exit 1
fi

echo "Rust is installed"

# Build the project
echo ""
echo "Building project..."
cargo build --release

if [ $? -eq 0 ]; then
  echo "Build successful"
else
  echo "Build failed"
  exit 1
fi

# Make import script executable
if [ -f "import_all.sh" ]; then
  chmod +x import_all.sh
  echo "Made import_all.sh executable"
fi

# Create output directory for example
mkdir -p data

echo ""
echo "Setup complete!"
echo ""
echo "Quick start:"
echo "  1. Import logs:  cargo run --release -- import your_log.log --db analytics.duckdb"
echo "  2. Start server: cargo run --release -- serve --db analytics.duckdb"
echo "  3. Open browser: http://localhost:8080"
echo ""
echo "For batch import:"
echo "  ./import_all.sh analytics.duckdb /path/to/logs/"
echo ""
