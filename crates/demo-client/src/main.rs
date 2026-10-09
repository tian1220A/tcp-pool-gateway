//! Demo client: drives `concurrency` keep-alive connections against the
//! gateway, each performing a sequence of request/response round-trips, and
//! reports throughput + latency. Run this after mock-server and gateway.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use tokio::net::TcpStream;

use protocol::{read_frame, write_frame};

struct Config {
    addr: SocketAddr,
    requests: u64,
    concurrency: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            addr: "127.0.0.1:8000".parse().unwrap(),
            requests: 10_000,
            concurrency: 200,
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
            "--addr" => { cfg.addr = val().parse().unwrap(); i += 2; }
            "--requests" => { cfg.requests = val().parse().unwrap(); i += 2; }
            "--concurrency" => { cfg.concurrency = val().parse().unwrap(); i += 2; }
            _ => { eprintln!("[demo-client] unknown arg: {key}"); i += 1; }
        }
    }
    cfg
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = parse_args();
    let per_conn = cfg.requests.div_ceil(cfg.concurrency);

    let done = Arc::new(AtomicU64::new(0));
    let errors = Arc::new(AtomicU64::new(0));
    let latency_ns = Arc::new(AtomicU64::new(0));

    let start = Instant::now();
    let mut handles = Vec::with_capacity(cfg.concurrency as usize);
    for _ in 0..cfg.concurrency {
        let addr = cfg.addr;
        let done = done.clone();
        let errors = errors.clone();
        let latency_ns = latency_ns.clone();
        handles.push(tokio::spawn(async move {
            let mut sock = match TcpStream::connect(addr).await {
                Ok(s) => s,
                Err(_) => {
                    errors.fetch_add(1, Ordering::Relaxed);
                    return;
                }
            };
            if sock.set_nodelay(true).is_err() {
                errors.fetch_add(1, Ordering::Relaxed);
                return;
            }
            for i in 0..per_conn {
                let payload = format!("req-{i}").into_bytes();
                let t0 = Instant::now();
                if write_frame(&mut sock, &payload).await.is_err() {
                    errors.fetch_add(1, Ordering::Relaxed);
                    break;
                }
                match read_frame(&mut sock).await {
                    Ok(Some(resp)) => {
                        let expected: Vec<u8> =
                            payload.iter().map(|b| b.to_ascii_uppercase()).collect();
                        if resp != expected {
                            errors.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                    _ => {
                        errors.fetch_add(1, Ordering::Relaxed);
                        break;
                    }
                }
                latency_ns.fetch_add(t0.elapsed().as_nanos() as u64, Ordering::Relaxed);
                done.fetch_add(1, Ordering::Relaxed);
            }
        }));
    }
    for h in handles {
        h.await?;
    }
    let elapsed = start.elapsed();

    let total = done.load(Ordering::Relaxed);
    let errs = errors.load(Ordering::Relaxed);
    println!("=== demo-client summary ===");
    println!("completed: {total} requests (concurrency {})", cfg.concurrency);
    println!("errors:    {errs}");
    println!("elapsed:   {elapsed:?}");
    if total > 0 {
        let secs = elapsed.as_secs_f64();
        let avg_ms = latency_ns.load(Ordering::Relaxed) as f64 / total as f64 / 1e6;
        println!("qps:        {:.0} req/s", total as f64 / secs);
        println!("avg latency: {:.2} ms", avg_ms);
    }
    Ok(())
}
