use anyhow::{anyhow, Result};
use chrono::{DateTime, FixedOffset};
use regex::Regex;
use serde::Serialize;
use std::sync::OnceLock;
use url::Url;

#[derive(Debug, Clone, Serialize)]
pub struct LogRow {
    pub remote_addr: String,
    pub identd: Option<String>,
    pub user_or_session: Option<String>,
    pub ts: DateTime<FixedOffset>,
    pub method: String,
    pub url: String,
    pub scheme: Option<String>,
    pub host: Option<String>,
    pub port: Option<i32>,
    pub path: Option<String>,
    pub query: Option<String>,
    pub http_version: String,
    pub status: i32,
    pub bytes: Option<i64>,
    pub country: Option<String>,
    pub user_agent: Option<String>,
    pub raw: String,
}

fn none_if_dash(s: &str) -> Option<String> {
    let t = s.trim();
    if t == "-" { None } else { Some(t.to_string()) }
}

// Example timestamp: 15/Feb/2026:00:00:04 +0000
fn parse_ts(ts: &str) -> Result<DateTime<FixedOffset>> {
    // chrono format: "%d/%b/%Y:%H:%M:%S %z"
    Ok(DateTime::parse_from_str(ts, "%d/%b/%Y:%H:%M:%S %z")?)
}

pub fn parse_line(line: &str) -> Result<LogRow> {
    // remote_addr SP identd SP user_or_session SP [ts] SP "METHOD URL HTTP/x" SP status SP bytes SP "country" SP "ua"
    // country may be e.g. "US", "TR", "VN", or "98"
    //
    // Capture groups:
    // 1 ip
    // 2 identd
    // 3 user/session
    // 4 timestamp
    // 5 method
    // 6 url
    // 7 http_version
    // 8 status
    // 9 bytes or -
    // 10 country
    // 11 user-agent
    //
    // NOTE: This assumes the request is fully quoted and country/ua are quoted.
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r#"^(\S+)\s+(\S+)\s+(\S+)\s+\[([^\]]+)\]\s+"(\S+)\s+(\S+)\s+([^"]+)"\s+(\d{3})\s+(\S+)\s+"([^"]*)"\s+"([^"]*)"\s*$"#)
            .expect("regex compiles")
    });

    let caps = re
        .captures(line)
        .ok_or_else(|| anyhow!("line did not match expected format"))?;

    let remote_addr = caps[1].to_string();
    let identd = none_if_dash(&caps[2]);
    let user_or_session = none_if_dash(&caps[3]);
    let ts = parse_ts(&caps[4])?;

    let method = caps[5].to_string();
    let url_str = caps[6].to_string();
    let http_version = caps[7].to_string();

    let status: i32 = caps[8].parse()?;

    let bytes = match &caps[9] {
        "-" => None,
        x => Some(x.parse::<i64>()?),
    };

    let country = {
        let c = caps[10].trim();
        if c.is_empty() { None } else { Some(c.to_string()) }
    };

    let user_agent = {
        let ua = caps[11].trim();
        if ua.is_empty() { None } else { Some(ua.to_string()) }
    };

    // Parse URL into components (best-effort; URL can be huge)
    let (scheme, host, port, path, query) = match Url::parse(&url_str) {
        Ok(u) => (
            Some(u.scheme().to_string()),
            u.host_str().map(|s| s.to_string()),
            u.port().map(|p| p as i32),
            Some(u.path().to_string()),
            u.query().map(|q| q.to_string()),
        ),
        Err(_) => (None, None, None, None, None),
    };

    Ok(LogRow {
        remote_addr,
        identd,
        user_or_session,
        ts,
        method,
        url: url_str,
        scheme,
        host,
        port,
        path,
        query,
        http_version,
        status,
        bytes,
        country,
        user_agent,
        raw: line.to_string(),
    })
}
