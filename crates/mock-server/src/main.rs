//! Mock backend business service: reads length-prefixed frames, uppercases the
//! payload (simulating "business work"), optionally adds latency, and replies.

use std::net::SocketAddr;
use std::time::Duration;

use tokio::net::{TcpListener, TcpStream};

use protocol::{read_frame, write_frame};

struct Config {
    listen: SocketAddr,
    latency_ms: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            listen: "0.0.0.0:9000".parse().unwrap(),
            latency_ms: 0,
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
            "--listen" => { cfg.listen = val().parse().unwrap(); i += 2; }
            "--latency-ms" => { cfg.latency_ms = val().parse().unwrap(); i += 2; }
            _ => { eprintln!("[mock-server] unknown arg: {key}"); i += 1; }
        }
    }
    cfg
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = parse_args();
    let listener = TcpListener::bind(cfg.listen).await?;
    println!(
        "[mock-server] listening on {} (latency {}ms)",
        listener.local_addr()?,
        cfg.latency_ms
    );

    loop {
        let (sock, peer) = listener.accept().await?;
        sock.set_nodelay(true)?;
        println!("[mock-server] accepted connection from {peer}");
        let latency_ms = cfg.latency_ms;
        tokio::spawn(async move {
            if let Err(e) = handle(sock, latency_ms).await {
                println!("[mock-server] connection {peer} closed: {e}");
            }
        });
    }
}

async fn handle(mut sock: TcpStream, latency_ms: u64) -> anyhow::Result<()> {
    loop {
        let Some(payload) = read_frame(&mut sock).await? else {
            break; // client closed
        };

        // Simulated business latency.
        if latency_ms > 0 {
            tokio::time::sleep(Duration::from_millis(latency_ms)).await;
        }

        // "Business logic": uppercase the ASCII payload.
        let resp: Vec<u8> = payload.iter().map(|b| b.to_ascii_uppercase()).collect();
        write_frame(&mut sock, &resp).await?;
    }
    Ok(())
}
