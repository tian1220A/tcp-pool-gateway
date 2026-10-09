//! TCP Pool Gateway — TCP proxy + HTTP admin API + frontend static hosting.

mod history;
mod http_api;
mod logging;
mod proxy;
mod state;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tower_http::services::ServeDir;
use tcp_pool::{Pool, PoolConfig};

use crate::history::HistoryPoint;
use crate::state::AppState;

struct Config {
    tcp_listen: SocketAddr,
    backend: SocketAddr,
    http_listen: SocketAddr,
    static_dir: PathBuf,
    max_connections: usize,
    idle_timeout_ms: u64,
    acquire_timeout_ms: u64,
    min_connections: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            tcp_listen: "0.0.0.0:8000".parse().unwrap(),
            backend: "127.0.0.1:9000".parse().unwrap(),
            http_listen: "0.0.0.0:8080".parse().unwrap(),
            static_dir: PathBuf::from("frontend/dist"),
            max_connections: 32,
            idle_timeout_ms: 30_000,
            acquire_timeout_ms: 5_000,
            min_connections: 0,
        }
    }
}

fn parse_args() -> Config {
    let mut cfg = Config::default();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        let key = args[i].clone();
        let val = || args.get(i + 1).cloned().unwrap_or_default();
        match key.as_str() {
            "--tcp-listen" => { cfg.tcp_listen = val().parse().unwrap(); i += 2; }
            "--backend" => { cfg.backend = val().parse().unwrap(); i += 2; }
            "--http-listen" => { cfg.http_listen = val().parse().unwrap(); i += 2; }
            "--static-dir" => { cfg.static_dir = PathBuf::from(val()); i += 2; }
            "--max-connections" => { cfg.max_connections = val().parse().unwrap(); i += 2; }
            "--idle-timeout-ms" => { cfg.idle_timeout_ms = val().parse().unwrap(); i += 2; }
            "--acquire-timeout-ms" => { cfg.acquire_timeout_ms = val().parse().unwrap(); i += 2; }
            "--min-connections" => { cfg.min_connections = val().parse().unwrap(); i += 2; }
            _ => {
                eprintln!("[gateway] unknown arg: {key}");
                i += 1;
            }
        }
    }
    cfg
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = parse_args();

    let pool = Pool::new(PoolConfig {
        max_connections: cfg.max_connections,
        idle_timeout: Duration::from_millis(cfg.idle_timeout_ms),
        acquire_timeout: Duration::from_millis(cfg.acquire_timeout_ms),
        target_addr: cfg.backend,
        min_connections: cfg.min_connections,
        connect_timeout: Duration::from_secs(3),
    });

    let state = Arc::new(AppState::new(pool));
    state.logs.push(
        "info",
        format!(
            "gateway starting: tcp={} backend={} http={}",
            cfg.tcp_listen, cfg.backend, cfg.http_listen
        ),
    );

    // Background sampler: record pool stats every second for the trend chart.
    {
        let sampler = state.clone();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(Duration::from_secs(1));
            loop {
                ticker.tick().await;
                sampler.history.push(HistoryPoint::sample(&sampler.pool.stats()));
            }
        });
    }

    let app = http_api::router()
        .with_state(state.clone())
        .fallback_service(ServeDir::new(&cfg.static_dir));

    let http_listener = tokio::net::TcpListener::bind(cfg.http_listen).await?;
    let http_addr = http_listener.local_addr()?;
    let http_state = state.clone();
    let http_task = tokio::spawn(async move {
        http_state.logs.push(
            "info",
            format!(
                "HTTP admin + frontend on http://{http_addr} (static dir: {})",
                cfg.static_dir.display()
            ),
        );
        if let Err(e) = axum::serve(http_listener, app).await {
            http_state.logs.push("error", format!("HTTP server error: {e}"));
        }
    });

    let tcp_task = tokio::spawn(proxy::run_tcp_server(cfg.tcp_listen, state.clone()));

    tokio::select! {
        _ = http_task => {}
        _ = tcp_task => {}
        _ = tokio::signal::ctrl_c() => {
            state.logs.push("info", "shutting down...");
            state.pool.close();
        }
    }

    Ok(())
}
