use std::{net::SocketAddr, sync::Arc};

use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    response::Html,
    routing::get,
};
use duckdb::{Connection, params};
use serde::Deserialize;
use serde_json::json;
use tower_http::cors::{Any, CorsLayer};

#[derive(Clone)]
pub struct AppState {
    pub db_path: Arc<String>,
}

type ApiResult<T> = Result<Json<T>, (StatusCode, String)>;

fn internal_error<E: std::fmt::Display>(e: E) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
}

fn with_conn<T>(
    db_path: &str,
    f: impl FnOnce(&Connection) -> anyhow::Result<T>,
) -> anyhow::Result<T> {
    let conn = Connection::open(db_path)?;
    Ok(f(&conn)?)
}

pub async fn serve(db_path: String, bind: SocketAddr) -> anyhow::Result<()> {
    let state = AppState {
        db_path: Arc::new(db_path),
    };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/", get(index))
        .route("/api/requests_over_time", get(requests_over_time))
        .route("/api/top_hosts", get(top_hosts))
        .route("/api/status_codes", get(status_codes))
        .route("/api/top_countries", get(top_countries))
        .route("/api/bandwidth_over_time", get(bandwidth_over_time))
        .route("/api/hourly_heatmap", get(hourly_heatmap))
        .route("/api/error_analysis", get(error_analysis))
        .route("/api/top_paths", get(top_paths))
        .route("/api/user_agents", get(user_agents))
        .layer(cors)
        .with_state(state);

    println!("Listening on http://{}", bind);
    let listener = tokio::net::TcpListener::bind(bind).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn index() -> Html<&'static str> {
    Html(INDEX_HTML)
}

#[derive(Debug, Deserialize)]
struct TimeParams {
    start: Option<String>,
    end: Option<String>,
}

async fn requests_over_time(
    State(st): State<AppState>,
    Query(q): Query<TimeParams>,
) -> ApiResult<serde_json::Value> {
    let db_path = st.db_path.clone();
    let payload = with_conn(&db_path, |conn| {
        let query = match (&q.start, &q.end) {
            (Some(_), Some(_)) => {
                r#"
                SELECT CAST(date_trunc('hour', ts) AS VARCHAR) AS t, count(*) AS n
                FROM requests
                WHERE ts >= CAST(? AS TIMESTAMPTZ) AND ts <= CAST(? AS TIMESTAMPTZ)
                GROUP BY 1 ORDER BY 1
            "#
            }
            (Some(_), None) => {
                r#"
                SELECT CAST(date_trunc('hour', ts) AS VARCHAR) AS t, count(*) AS n
                FROM requests
                WHERE ts >= CAST(? AS TIMESTAMPTZ)
                GROUP BY 1 ORDER BY 1
            "#
            }
            (None, Some(_)) => {
                r#"
                SELECT CAST(date_trunc('hour', ts) AS VARCHAR) AS t, count(*) AS n
                FROM requests
                WHERE ts <= CAST(? AS TIMESTAMPTZ)
                GROUP BY 1 ORDER BY 1
            "#
            }
            (None, None) => {
                r#"
                SELECT CAST(date_trunc('hour', ts) AS VARCHAR) AS t, count(*) AS n
                FROM requests
                GROUP BY 1 ORDER BY 1
                LIMIT 200
            "#
            }
        };

        let mut stmt = conn.prepare(query)?;
        let mut rows = match (&q.start, &q.end) {
            (Some(s), Some(e)) => stmt.query(params![s, e])?,
            (Some(s), None) => stmt.query(params![s])?,
            (None, Some(e)) => stmt.query(params![e])?,
            (None, None) => stmt.query(params![])?,
        };

        let mut out = Vec::new();
        while let Some(r) = rows.next()? {
            let t: String = r.get(0)?;
            let n: i64 = r.get(1)?;
            out.push(json!({"t": t, "n": n}));
        }
        Ok(json!({ "series": out }))
    })
    .map_err(internal_error)?;

    Ok(Json(payload))
}

async fn top_hosts(
    State(st): State<AppState>,
    Query(q): Query<TimeParams>,
) -> ApiResult<serde_json::Value> {
    let db_path = st.db_path.clone();
    let payload = with_conn(&db_path, |conn| {
        let query = match (&q.start, &q.end) {
            (Some(_), Some(_)) => r#"
                SELECT host, count(*) AS n FROM requests
                WHERE host IS NOT NULL AND ts >= CAST(? AS TIMESTAMPTZ) AND ts <= CAST(? AS TIMESTAMPTZ)
                GROUP BY 1 ORDER BY n DESC LIMIT 15
            "#,
            (Some(_), None) => r#"
                SELECT host, count(*) AS n FROM requests
                WHERE host IS NOT NULL AND ts >= CAST(? AS TIMESTAMPTZ)
                GROUP BY 1 ORDER BY n DESC LIMIT 15
            "#,
            (None, Some(_)) => r#"
                SELECT host, count(*) AS n FROM requests
                WHERE host IS NOT NULL AND ts <= CAST(? AS TIMESTAMPTZ)
                GROUP BY 1 ORDER BY n DESC LIMIT 15
            "#,
            (None, None) => r#"
                SELECT host, count(*) AS n FROM requests
                WHERE host IS NOT NULL
                GROUP BY 1 ORDER BY n DESC LIMIT 15
            "#,
        };

        let mut stmt = conn.prepare(query)?;
        let mut rows = match (&q.start, &q.end) {
            (Some(s), Some(e)) => stmt.query(params![s, e])?,
            (Some(s), None) => stmt.query(params![s])?,
            (None, Some(e)) => stmt.query(params![e])?,
            (None, None) => stmt.query(params![])?,
        };

        let mut out = Vec::new();
        while let Some(r) = rows.next()? {
            let host: String = r.get(0)?;
            let n: i64 = r.get(1)?;
            out.push(json!({"host": host, "n": n}));
        }
        Ok(json!({ "hosts": out }))
    }).map_err(internal_error)?;

    Ok(Json(payload))
}

async fn status_codes(
    State(st): State<AppState>,
    Query(q): Query<TimeParams>,
) -> ApiResult<serde_json::Value> {
    let db_path = st.db_path.clone();
    let payload = with_conn(&db_path, |conn| {
        let query = match (&q.start, &q.end) {
            (Some(_), Some(_)) => {
                r#"
                SELECT status, count(*) AS n FROM requests
                WHERE ts >= CAST(? AS TIMESTAMPTZ) AND ts <= CAST(? AS TIMESTAMPTZ)
                GROUP BY 1 ORDER BY n DESC
            "#
            }
            (Some(_), None) => {
                r#"
                SELECT status, count(*) AS n FROM requests
                WHERE ts >= CAST(? AS TIMESTAMPTZ)
                GROUP BY 1 ORDER BY n DESC
            "#
            }
            (None, Some(_)) => {
                r#"
                SELECT status, count(*) AS n FROM requests
                WHERE ts <= CAST(? AS TIMESTAMPTZ)
                GROUP BY 1 ORDER BY n DESC
            "#
            }
            (None, None) => {
                r#"
                SELECT status, count(*) AS n FROM requests
                GROUP BY 1 ORDER BY n DESC
            "#
            }
        };

        let mut stmt = conn.prepare(query)?;
        let mut rows = match (&q.start, &q.end) {
            (Some(s), Some(e)) => stmt.query(params![s, e])?,
            (Some(s), None) => stmt.query(params![s])?,
            (None, Some(e)) => stmt.query(params![e])?,
            (None, None) => stmt.query(params![])?,
        };

        let mut out = Vec::new();
        while let Some(r) = rows.next()? {
            let status: i32 = r.get(0)?;
            let n: i64 = r.get(1)?;
            out.push(json!({"status": status, "n": n}));
        }
        Ok(json!({ "status": out }))
    })
    .map_err(internal_error)?;

    Ok(Json(payload))
}

async fn top_countries(
    State(st): State<AppState>,
    Query(q): Query<TimeParams>,
) -> ApiResult<serde_json::Value> {
    let db_path = st.db_path.clone();
    let payload = with_conn(&db_path, |conn| {
        let query = match (&q.start, &q.end) {
            (Some(_), Some(_)) => {
                r#"
                SELECT country, count(*) AS n FROM requests
                WHERE country IS NOT NULL AND country <> ''
                  AND ts >= CAST(? AS TIMESTAMPTZ) AND ts <= CAST(? AS TIMESTAMPTZ)
                GROUP BY 1 ORDER BY n DESC LIMIT 20
            "#
            }
            (Some(_), None) => {
                r#"
                SELECT country, count(*) AS n FROM requests
                WHERE country IS NOT NULL AND country <> ''
                  AND ts >= CAST(? AS TIMESTAMPTZ)
                GROUP BY 1 ORDER BY n DESC LIMIT 20
            "#
            }
            (None, Some(_)) => {
                r#"
                SELECT country, count(*) AS n FROM requests
                WHERE country IS NOT NULL AND country <> ''
                  AND ts <= CAST(? AS TIMESTAMPTZ)
                GROUP BY 1 ORDER BY n DESC LIMIT 20
            "#
            }
            (None, None) => {
                r#"
                SELECT country, count(*) AS n FROM requests
                WHERE country IS NOT NULL AND country <> ''
                GROUP BY 1 ORDER BY n DESC LIMIT 20
            "#
            }
        };

        let mut stmt = conn.prepare(query)?;
        let mut rows = match (&q.start, &q.end) {
            (Some(s), Some(e)) => stmt.query(params![s, e])?,
            (Some(s), None) => stmt.query(params![s])?,
            (None, Some(e)) => stmt.query(params![e])?,
            (None, None) => stmt.query(params![])?,
        };

        let mut out = Vec::new();
        while let Some(r) = rows.next()? {
            let country: String = r.get(0)?;
            let n: i64 = r.get(1)?;
            out.push(json!({"country": country, "n": n}));
        }
        Ok(json!({ "countries": out }))
    })
    .map_err(internal_error)?;

    Ok(Json(payload))
}

async fn bandwidth_over_time(
    State(st): State<AppState>,
    Query(q): Query<TimeParams>,
) -> ApiResult<serde_json::Value> {
    let db_path = st.db_path.clone();
    let payload = with_conn(&db_path, |conn| {
        let query = match (&q.start, &q.end) {
            (None, None) => r#"
                SELECT 
                    CAST(date_trunc('hour', ts) AS VARCHAR) AS t,
                    CAST(SUM(COALESCE(bytes, 0)) / 1024.0 / 1024.0 AS BIGINT) AS mb
                FROM requests
                GROUP BY 1 ORDER BY 1 LIMIT 200
            "#,
            (Some(_), _) => r#"
                SELECT 
                    CAST(date_trunc('hour', ts) AS VARCHAR) AS t,
                    CAST(SUM(COALESCE(bytes, 0)) / 1024.0 / 1024.0 AS BIGINT) AS mb
                FROM requests
                WHERE ts >= CAST(? AS TIMESTAMPTZ)
                GROUP BY 1 ORDER BY 1
            "#,
            _ => r#"
                SELECT 
                    CAST(date_trunc('hour', ts) AS VARCHAR) AS t,
                    CAST(SUM(COALESCE(bytes, 0)) / 1024.0 / 1024.0 AS BIGINT) AS mb
                FROM requests
                GROUP BY 1 ORDER BY 1 LIMIT 200
            "#,
        };

        let mut stmt = conn.prepare(query)?;
        let mut rows = match (&q.start, &q.end) {
            (None, None) => stmt.query(params![])?,
            (Some(s), _) => stmt.query(params![s])?,
            _ => stmt.query(params![])?,
        };

        let mut out = Vec::new();
        while let Some(r) = rows.next()? {
            let t: String = r.get(0)?;
            let mb: i64 = r.get(1)?;
            out.push(json!({"t": t, "mb": mb}));
        }
        Ok(json!({ "series": out }))
    }).map_err(internal_error)?;

    Ok(Json(payload))
}

async fn hourly_heatmap(
    State(st): State<AppState>,
) -> ApiResult<serde_json::Value> {
    let db_path = st.db_path.clone();
    let payload = with_conn(&db_path, |conn| {
        let mut stmt = conn.prepare(
            r#"
            SELECT 
                CAST(EXTRACT(hour FROM ts) AS INTEGER) AS hour,
                CAST(EXTRACT(dow FROM ts) AS INTEGER) AS day_of_week,
                COUNT(*) AS n
            FROM requests
            GROUP BY 1, 2
            ORDER BY 1, 2
            "#,
        )?;

        let mut rows = stmt.query(params![])?;
        let mut out = Vec::new();
        while let Some(r) = rows.next()? {
            let hour: i32 = r.get(0)?;
            let dow: i32 = r.get(1)?;
            let n: i64 = r.get(2)?;
            out.push(json!({"hour": hour, "day": dow, "n": n}));
        }
        Ok(json!({ "data": out }))
    }).map_err(internal_error)?;

    Ok(Json(payload))
}

async fn error_analysis(
    State(st): State<AppState>,
) -> ApiResult<serde_json::Value> {
    let db_path = st.db_path.clone();
    let payload = with_conn(&db_path, |conn| {
        let mut stmt = conn.prepare(
            r#"
            SELECT 
                host,
                COUNT(*) AS errors,
                SUM(CASE WHEN status >= 500 THEN 1 ELSE 0 END) AS server_errors,
                SUM(CASE WHEN status >= 400 AND status < 500 THEN 1 ELSE 0 END) AS client_errors
            FROM requests
            WHERE status >= 400
            GROUP BY 1
            ORDER BY 2 DESC
            LIMIT 10
            "#,
        )?;

        let mut rows = stmt.query(params![])?;
        let mut out = Vec::new();
        while let Some(r) = rows.next()? {
            let host: String = r.get(0)?;
            let errors: i64 = r.get(1)?;
            let server_errors: i64 = r.get(2)?;
            let client_errors: i64 = r.get(3)?;
            out.push(json!({
                "host": host, 
                "errors": errors,
                "server_errors": server_errors,
                "client_errors": client_errors
            }));
        }
        Ok(json!({ "hosts": out }))
    }).map_err(internal_error)?;

    Ok(Json(payload))
}

async fn top_paths(
    State(st): State<AppState>,
) -> ApiResult<serde_json::Value> {
    let db_path = st.db_path.clone();
    let payload = with_conn(&db_path, |conn| {
        let mut stmt = conn.prepare(
            r#"
            SELECT 
                path,
                COUNT(*) AS n,
                AVG(COALESCE(bytes, 0)) AS avg_bytes
            FROM requests
            WHERE path IS NOT NULL AND path <> '/'
            GROUP BY 1
            ORDER BY 2 DESC
            LIMIT 15
            "#,
        )?;

        let mut rows = stmt.query(params![])?;
        let mut out = Vec::new();
        while let Some(r) = rows.next()? {
            let path: String = r.get(0)?;
            let n: i64 = r.get(1)?;
            let avg_bytes: f64 = r.get(2)?;
            out.push(json!({
                "path": path, 
                "n": n,
                "avg_kb": (avg_bytes / 1024.0) as i64
            }));
        }
        Ok(json!({ "paths": out }))
    }).map_err(internal_error)?;

    Ok(Json(payload))
}

async fn user_agents(
    State(st): State<AppState>,
) -> ApiResult<serde_json::Value> {
    let db_path = st.db_path.clone();
    let payload = with_conn(&db_path, |conn| {
        let mut stmt = conn.prepare(
            r#"
            SELECT 
                CASE 
                    WHEN user_agent LIKE '%Chrome%' AND user_agent NOT LIKE '%Edg%' THEN 'Chrome'
                    WHEN user_agent LIKE '%Firefox%' THEN 'Firefox'
                    WHEN user_agent LIKE '%Safari%' AND user_agent NOT LIKE '%Chrome%' THEN 'Safari'
                    WHEN user_agent LIKE '%Edg%' THEN 'Edge'
                    WHEN user_agent LIKE '%Opera%' THEN 'Opera'
                    WHEN user_agent LIKE '%bot%' OR user_agent LIKE '%Bot%' THEN 'Bot'
                    ELSE 'Other'
                END AS browser,
                COUNT(*) AS n
            FROM requests
            WHERE user_agent IS NOT NULL
            GROUP BY 1
            ORDER BY 2 DESC
            "#,
        )?;

        let mut rows = stmt.query(params![])?;
        let mut out = Vec::new();
        while let Some(r) = rows.next()? {
            let browser: String = r.get(0)?;
            let n: i64 = r.get(1)?;
            out.push(json!({"browser": browser, "n": n}));
        }
        Ok(json!({ "browsers": out }))
    }).map_err(internal_error)?;

    Ok(Json(payload))
}

const INDEX_HTML: &str = r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>EZproxy Analytics Dashboard</title>
    <script src="https://cdn.jsdelivr.net/npm/chart.js@4.4.0/dist/chart.umd.min.js"></script>
    <style>
        * { margin: 0; padding: 0; box-sizing: border-box; }
        body {
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            min-height: 100vh;
            padding: 20px;
        }
        .container { max-width: 1600px; margin: 0 auto; }
        h1 {
            color: white;
            font-size: 2.5rem;
            margin-bottom: 10px;
            text-shadow: 2px 2px 4px rgba(0,0,0,0.2);
        }
        .subtitle {
            color: rgba(255,255,255,0.9);
            font-size: 1.1rem;
            margin-bottom: 30px;
        }
        .grid {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(450px, 1fr));
            gap: 20px;
            margin-bottom: 20px;
        }
        .card {
            background: white;
            padding: 25px;
            border-radius: 12px;
            box-shadow: 0 10px 30px rgba(0,0,0,0.2);
            transition: transform 0.3s ease;
        }
        .card:hover { transform: translateY(-5px); }
        .card h2 {
            margin: 0 0 20px 0;
            font-size: 1.3rem;
            color: #333;
            border-bottom: 3px solid #667eea;
            padding-bottom: 10px;
        }
        .chart-container {
            position: relative;
            height: 300px;
            margin-top: 10px;
        }
        .stat-list {
            list-style: none;
            padding: 0;
            max-height: 400px;
            overflow-y: auto;
        }
        .stat-item {
            display: flex;
            justify-content: space-between;
            align-items: center;
            padding: 12px;
            margin-bottom: 8px;
            background: #f8f9fa;
            border-radius: 6px;
            border-left: 4px solid #667eea;
        }
        .stat-label { 
            font-weight: 500; 
            color: #333;
            flex: 1;
            overflow: hidden;
            text-overflow: ellipsis;
            white-space: nowrap;
            margin-right: 10px;
        }
        .stat-value {
            font-weight: bold;
            color: #667eea;
            background: #e7e9fc;
            padding: 4px 12px;
            border-radius: 4px;
            white-space: nowrap;
        }
        .error-value {
            background: #fee;
            color: #dc2626;
        }
        .loading {
            text-align: center;
            padding: 40px;
            color: #999;
            font-style: italic;
        }
    </style>
</head>
<body>
    <div class="container">
        <h1>EZproxy Analytics Dashboard</h1>
        <p class="subtitle">Real-time proxy usage insights and performance metrics</p>

        <div class="grid">
            <div class="card">
                <h2>Requests Over Time</h2>
                <div class="chart-container">
                    <canvas id="timeChart"></canvas>
                </div>
            </div>

            <div class="card">
                <h2>💾 Bandwidth Usage (MB/hour)</h2>
                <div class="chart-container">
                    <canvas id="bandwidthChart"></canvas>
                </div>
            </div>

            <div class="card">
                <h2>Top Hosts</h2>
                <ul id="top-hosts" class="stat-list loading">Loading...</ul>
            </div>

            <div class="card">
                <h2>Status Codes Distribution</h2>
                <div class="chart-container">
                    <canvas id="statusChart"></canvas>
                </div>
            </div>

            <div class="card">
                <h2>Top Countries</h2>
                <div class="chart-container">
                    <canvas id="countryChart"></canvas>
                </div>
            </div>

            <div class="card">
                <h2>Usage Heatmap (Hour × Day)</h2>
                <div class="chart-container">
                    <canvas id="heatmapChart"></canvas>
                </div>
            </div>

            <div class="card">
                <h2>Top Errors by Host</h2>
                <ul id="error-list" class="stat-list loading">Loading...</ul>
            </div>

            <div class="card">
                <h2>Browser Distribution</h2>
                <div class="chart-container">
                    <canvas id="browserChart"></canvas>
                </div>
            </div>

            <div class="card">
                <h2>Most Accessed Paths</h2>
                <ul id="path-list" class="stat-list loading">Loading...</ul>
            </div>
        </div>
    </div>

    <script>
        async function fetchData(endpoint, elementId, renderFn) {
            try {
                const res = await fetch(endpoint);
                const data = await res.json();
                renderFn(data);
            } catch (e) {
                const el = document.getElementById(elementId);
                if (el) el.innerHTML = '<div class="loading">Error loading data</div>';
                console.error('Error:', e);
            }
        }

        function renderTopHosts(data) {
            const container = document.getElementById('top-hosts');
            const hosts = data.hosts || [];

            if (hosts.length === 0) {
                container.innerHTML = '<div class="loading">No data available</div>';
                return;
            }

            container.innerHTML = hosts.map(item => `
                <li class="stat-item">
                    <span class="stat-label" title="${item.host}">${item.host}</span>
                    <span class="stat-value">${item.n.toLocaleString()}</span>
                </li>
            `).join('');
        }

        function renderTimeSeries(data) {
            const series = data.series || [];
            const ctx = document.getElementById('timeChart').getContext('2d');

            new Chart(ctx, {
                type: 'line',
                data: {
                    labels: series.map(d => {
                        const date = new Date(d.t);
                        return date.toLocaleDateString() + ' ' + date.getHours() + ':00';
                    }),
                    datasets: [{
                        label: 'Requests',
                        data: series.map(d => d.n),
                        borderColor: '#667eea',
                        backgroundColor: 'rgba(102, 126, 234, 0.1)',
                        tension: 0.4,
                        fill: true
                    }]
                },
                options: {
                    responsive: true,
                    maintainAspectRatio: false,
                    plugins: {
                        legend: { display: false }
                    },
                    scales: {
                        y: { beginAtZero: true }
                    }
                }
            });
        }

        function renderStatusCodes(data) {
            const statuses = data.status || [];
            const ctx = document.getElementById('statusChart').getContext('2d');

            const groups = { '2xx': 0, '3xx': 0, '4xx': 0, '5xx': 0, 'Other': 0 };
            statuses.forEach(item => {
                const code = Math.floor(item.status / 100);
                const key = code >= 2 && code <= 5 ? `${code}xx` : 'Other';
                groups[key] += item.n;
            });

            new Chart(ctx, {
                type: 'doughnut',
                data: {
                    labels: Object.keys(groups),
                    datasets: [{
                        data: Object.values(groups),
                        backgroundColor: [
                            '#10b981',
                            '#3b82f6',
                            '#f59e0b',
                            '#ef4444',
                            '#6b7280'
                        ]
                    }]
                },
                options: {
                    responsive: true,
                    maintainAspectRatio: false,
                    plugins: {
                        legend: {
                            position: 'bottom'
                        }
                    }
                }
            });
        }

        function renderCountries(data) {
            const countries = data.countries || [];
            const ctx = document.getElementById('countryChart').getContext('2d');

            new Chart(ctx, {
                type: 'bar',
                data: {
                    labels: countries.slice(0, 10).map(c => c.country),
                    datasets: [{
                        label: 'Requests',
                        data: countries.slice(0, 10).map(c => c.n),
                        backgroundColor: 'rgba(102, 126, 234, 0.8)',
                        borderColor: '#667eea',
                        borderWidth: 1
                    }]
                },
                options: {
                    indexAxis: 'y',
                    responsive: true,
                    maintainAspectRatio: false,
                    plugins: {
                        legend: { display: false }
                    },
                    scales: {
                        x: { beginAtZero: true }
                    }
                }
            });
        }

        function renderBandwidth(data) {
            const series = data.series || [];
            const ctx = document.getElementById('bandwidthChart').getContext('2d');

            new Chart(ctx, {
                type: 'bar',
                data: {
                    labels: series.map(d => {
                        const date = new Date(d.t);
                        return date.toLocaleDateString() + ' ' + date.getHours() + ':00';
                    }),
                    datasets: [{
                        label: 'Bandwidth (MB)',
                        data: series.map(d => d.mb),
                        backgroundColor: 'rgba(16, 185, 129, 0.6)',
                        borderColor: '#10b981',
                        borderWidth: 1
                    }]
                },
                options: {
                    responsive: true,
                    maintainAspectRatio: false,
                    plugins: { legend: { display: false } },
                    scales: { y: { beginAtZero: true } }
                }
            });
        }

        function renderHeatmap(data) {
            const heatmapData = data.data || [];
            const ctx = document.getElementById('heatmapChart').getContext('2d');

            const days = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];
            
            const dayData = days.map((day, dayIdx) => {
                const dayTotal = heatmapData
                    .filter(d => d.day === dayIdx)
                    .reduce((sum, d) => sum + d.n, 0);
                return dayTotal;
            });

            new Chart(ctx, {
                type: 'bar',
                data: {
                    labels: days,
                    datasets: [{
                        label: 'Requests by Day',
                        data: dayData,
                        backgroundColor: [
                            'rgba(102, 126, 234, 0.4)',
                            'rgba(102, 126, 234, 0.5)',
                            'rgba(102, 126, 234, 0.6)',
                            'rgba(102, 126, 234, 0.7)',
                            'rgba(102, 126, 234, 0.8)',
                            'rgba(102, 126, 234, 0.9)',
                            'rgba(102, 126, 234, 0.4)'
                        ],
                        borderColor: '#667eea',
                        borderWidth: 1
                    }]
                },
                options: {
                    responsive: true,
                    maintainAspectRatio: false,
                    plugins: { legend: { display: false } },
                    scales: { y: { beginAtZero: true } }
                }
            });
        }

        function renderErrors(data) {
            const container = document.getElementById('error-list');
            const hosts = data.hosts || [];

            if (hosts.length === 0) {
                container.innerHTML = '<div class="loading">No errors found</div>';
                return;
            }

            container.innerHTML = hosts.map(item => `
                <li class="stat-item">
                    <span class="stat-label" title="${item.host}">${item.host}</span>
                    <span class="stat-value error-value">
                        ${item.errors} (5xx: ${item.server_errors}, 4xx: ${item.client_errors})
                    </span>
                </li>
            `).join('');
        }

        function renderBrowsers(data) {
            const browsers = data.browsers || [];
            const ctx = document.getElementById('browserChart').getContext('2d');

            new Chart(ctx, {
                type: 'pie',
                data: {
                    labels: browsers.map(b => b.browser),
                    datasets: [{
                        data: browsers.map(b => b.n),
                        backgroundColor: [
                            '#3b82f6',
                            '#10b981',
                            '#f59e0b',
                            '#ef4444',
                            '#8b5cf6',
                            '#ec4899',
                            '#6b7280'
                        ]
                    }]
                },
                options: {
                    responsive: true,
                    maintainAspectRatio: false,
                    plugins: {
                        legend: {
                            position: 'bottom'
                        }
                    }
                }
            });
        }

        function renderPaths(data) {
            const container = document.getElementById('path-list');
            const paths = data.paths || [];

            if (paths.length === 0) {
                container.innerHTML = '<div class="loading">No data available</div>';
                return;
            }

            container.innerHTML = paths.map(item => `
                <li class="stat-item">
                    <span class="stat-label" title="${item.path}">${item.path}</span>
                    <span class="stat-value">${item.n.toLocaleString()} (${item.avg_kb}KB)</span>
                </li>
            `).join('');
        }

        fetchData('/api/top_hosts', 'top-hosts', renderTopHosts);
        fetchData('/api/requests_over_time', 'timeChart', renderTimeSeries);
        fetchData('/api/status_codes', 'statusChart', renderStatusCodes);
        fetchData('/api/top_countries', 'countryChart', renderCountries);
        fetchData('/api/bandwidth_over_time', 'bandwidthChart', renderBandwidth);
        fetchData('/api/hourly_heatmap', 'heatmapChart', renderHeatmap);
        fetchData('/api/error_analysis', 'error-list', renderErrors);
        fetchData('/api/user_agents', 'browserChart', renderBrowsers);
        fetchData('/api/top_paths', 'path-list', renderPaths);
    </script>
</body>
</html>
"#;
