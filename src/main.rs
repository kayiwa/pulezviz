// src/main.rs
mod db;
mod parser;
mod web;

use std::{fs::File, io::{BufRead, BufReader}, net::SocketAddr};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "ezvis")]
#[command(about = "Ezproxy log -> DuckDB -> dashboard", long_about = None)]
struct Cli {
    #[command(subcommand)]
    cmd: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Import a log file into DuckDB
    Import {
        /// Path to log file
        log_path: String,

        /// DuckDB database file
        #[arg(long, default_value = "ezvis.duckdb")]
        db: String,
    },

    /// Run a local dashboard server
    Serve {
        /// DuckDB database file
        #[arg(long, default_value = "ezvis.duckdb")]
        db: String,

        /// Bind address
        #[arg(long, default_value = "127.0.0.1:8080")]
        bind: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.cmd {
        Command::Import { log_path, db } => {
            // FIX 1: conn must be mutable to start a transaction later
            let mut conn = db::open_db(&db)?; 
            db::init_schema(&conn)?;

            let f = File::open(&log_path).with_context(|| format!("open {}", log_path))?;
            let rdr = BufReader::new(f);

            let rows = rdr.lines().filter_map(|line| {
                let line = match line {
                    Ok(l) => l,
                    Err(_) => return None,
                };
                match parser::parse_line(&line) {
                    Ok(r) => Some(r),
                    Err(_) => None,
                }
            });

            // FIX 2: pass &mut conn
            let (ok, bad) = db::insert_rows(&mut conn, rows)?; 
            println!("import complete: ok={} bad={}", ok, bad);
        }

        Command::Serve { db, bind } => {
            let bind: SocketAddr = bind.parse().context("parse bind addr")?;
            web::serve(db, bind).await?;
        }
    }

    Ok(())
}
