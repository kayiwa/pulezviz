use anyhow::Result;
use duckdb::{params, Connection};
use crate::parser::LogRow;

pub fn open_db(path: &str) -> Result<Connection> {
    Ok(Connection::open(path)?)
}

pub fn init_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS requests (
          ts TIMESTAMPTZ,
          remote_addr TEXT,
          identd TEXT,
          user_or_session TEXT,
          method TEXT,
          url TEXT,
          scheme TEXT,
          host TEXT,
          port INTEGER,
          path TEXT,
          query TEXT,
          http_version TEXT,
          status INTEGER,
          bytes BIGINT,
          country TEXT,
          user_agent TEXT,
          raw TEXT
        );

        CREATE INDEX IF NOT EXISTS idx_requests_ts ON requests(ts);
        CREATE INDEX IF NOT EXISTS idx_requests_host ON requests(host);
        CREATE INDEX IF NOT EXISTS idx_requests_status ON requests(status);
        CREATE INDEX IF NOT EXISTS idx_requests_country ON requests(country);
        "#,
    )?;
    Ok(())
}

pub fn insert_rows(conn: &mut Connection, rows: impl Iterator<Item = LogRow>) -> Result<(u64, u64)> {
    let mut ok: u64 = 0;
    let mut bad: u64 = 0;
    let rows_vec: Vec<LogRow> = rows.collect();
    let total = rows_vec.len();
    println!("Processing {} log entries...", total);
    // Use DuckDB's appender for much faster bulk inserts
    // This is the recommended way for bulk loading in DuckDB
    let mut appender = conn.appender("requests")?;
    for (idx, r) in rows_vec.iter().enumerate() {
        let ts = r.ts.to_rfc3339();

        let res = appender.append_row(params![
            ts,
            &r.remote_addr,
            &r.identd,
            &r.user_or_session,
            &r.method,
            &r.url,
            &r.scheme,
            &r.host,
            r.port,
            &r.path,
            &r.query,
            &r.http_version,
            r.status,
            r.bytes,
            &r.country,
            &r.user_agent,
            &r.raw
        ]);

        match res {
            Ok(_) => ok += 1,
            Err(e) => {
                bad += 1;
                eprintln!("Row {} failed: {}", idx + 1, e);
            }
        }
        
        if (idx + 1) % 10000 == 0 {
            println!("  Processed {} / {} entries ({} ok, {} failed)", idx + 1, total, ok, bad);
        }
    }
    
    // Flush the appender
    let _ = appender.flush();
    
    println!("Import complete!");
    
    Ok((ok, bad))
}
