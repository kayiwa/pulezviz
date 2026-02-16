#!/bin/bash

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
