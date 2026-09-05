use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use hns_core::*;
use std::{
    io::{Read, Write},
    path::PathBuf,
};
use subtle::ConstantTimeEq;
use tiny_http::{Header, Response, Server, StatusCode};

#[derive(Parser)]
#[command(
    version,
    about = "Local home-network observations. Capture and discovery run only when explicitly requested."
)]
struct Args {
    #[arg(long, default_value = ".data/network.db", global = true)]
    db: PathBuf,
    #[arg(long, default_value = "local", global = true)]
    sensor: String,
    #[command(subcommand)]
    command: Action,
}
#[derive(Subcommand)]
enum Action {
    /// Print a snapshot. Demo data is isolated in memory.
    Snapshot {
        #[arg(long)]
        demo: bool,
    },
    /// Import NDJSON observations or PCAP/PCAPNG via separately installed TShark.
    Import { file: PathBuf },
    /// Configure local IPv4/IPv6 prefixes. Include your globally routed IPv6 prefix.
    Networks { cidrs: String },
    /// Discover hosts in a private IPv4 /24 or smaller. Sends probes.
    Discover { cidr: String },
    /// Capture one interface through TShark. Requires capture permissions. Does not change the network.
    Capture {
        #[arg(long)]
        interface: String,
        #[arg(long,default_value_t=60,value_parser=clap::value_parser!(u64).range(1..=86400))]
        seconds: u64,
    },
    /// Serve the authenticated API on loopback. Use an SSH tunnel for remote access.
    Serve {
        #[arg(long, default_value_t = 9898)]
        port: u16,
        #[arg(long, default_value = ".data/collector.token")]
        token_file: PathBuf,
        #[arg(long)]
        demo: bool,
    },
    /// List installed capture/discovery tools without running a scan or capture.
    Doctor,
}

fn main() -> Result<()> {
    let args = Args::parse();
    identifier(&args.sensor)?;
    if let Action::Doctor = args.command {
        for binary in ["tshark", "dumpcap", "nmap"] {
            let status = std::process::Command::new(binary).arg("--version").output();
            println!(
                "{binary}: {}",
                if status.is_ok_and(|o| o.status.success()) {
                    "available"
                } else {
                    "not available"
                }
            );
        }
        println!(
            "No traffic captured or hosts scanned. Coverage requires suitable sensor placement."
        );
        return Ok(());
    }
    let demo = matches!(
        args.command,
        Action::Snapshot { demo: true } | Action::Serve { demo: true, .. }
    );
    let mut store = if demo {
        demo_store()?
    } else {
        Store::open(&args.db)?
    };
    if !demo && !store.sensors()?.iter().any(|s| s.id == args.sensor) {
        store.set_sensor(&Sensor::new(&args.sensor, "local"))?;
    }
    match args.command {
        Action::Snapshot { .. } => println!(
            "{}",
            serde_json::to_string_pretty(&store.snapshot(
                if demo {
                    Some("sample")
                } else {
                    Some(&args.sensor)
                },
                if demo { "demo" } else { "local" }
            )?)?
        ),
        Action::Import { file } => {
            let count = if file
                .extension()
                .is_some_and(|e| e == "pcap" || e == "pcapng")
            {
                import_pcap(&mut store, &file, &args.sensor)?
            } else {
                if std::fs::metadata(&file)?.len() > 32 * 1024 * 1024 {
                    bail!("NDJSON import exceeds 32 MiB");
                }
                store.import_json(&std::fs::read_to_string(file)?, &args.sensor)?
            };
            println!("Imported {count} new observations; duplicate IDs are ignored.");
        }
        Action::Networks { cidrs } => {
            store.set_networks(&cidrs)?;
            println!("Local prefixes saved.");
        }
        Action::Discover { cidr } => {
            let found = discover(&cidr)?;
            store.save_discovery(&args.sensor, &found)?;
            println!("{}", serde_json::to_string_pretty(&found)?);
        }
        Action::Capture { interface, seconds } => {
            let count = capture(
                &mut store,
                &args.sensor,
                &interface,
                seconds,
                &std::sync::atomic::AtomicBool::new(false),
                |_| {},
            )?;
            println!("Recorded {count} IP observations. Packet drops remain unknown.");
        }
        Action::Serve {
            port, token_file, ..
        } => serve(store, port, &token_file, demo)?,
        Action::Doctor => unreachable!(),
    }
    Ok(())
}

fn load_token(path: &PathBuf) -> Result<String> {
    if path.exists() {
        let token = std::fs::read_to_string(path)?.trim().to_string();
        if token.len() < 32 {
            bail!("Collector token must contain at least 32 characters");
        }
        return Ok(token);
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let token = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut file = opts.open(path)?;
    file.write_all(token.as_bytes())?;
    Ok(token)
}

fn serve(store: Store, port: u16, token_file: &PathBuf, demo: bool) -> Result<()> {
    let token = load_token(token_file)?;
    let server = Server::http((std::net::Ipv4Addr::LOCALHOST, port))
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    eprintln!(
        "Collector API: http://127.0.0.1:{port}; token file: {}. No capture started.",
        token_file.display()
    );
    for mut req in server.incoming_requests() {
        let authenticated = req
            .headers()
            .iter()
            .find(|h| h.field.equiv("Authorization"))
            .is_some_and(|h| {
                let expected = format!("Bearer {token}");
                bool::from(h.value.as_str().as_bytes().ct_eq(expected.as_bytes()))
            });
        let (status, body) = if !authenticated {
            (401, serde_json::json!({"error":"Unauthorized"}))
        } else {
            let result = (|| -> Result<serde_json::Value> {
                match (
                    req.method().as_str(),
                    req.url().split('?').next().unwrap_or(""),
                ) {
                    ("GET", "/v1/snapshot") => {
                        let params: std::collections::HashMap<_, _> = req
                            .url()
                            .split_once('?')
                            .map(|(_, q)| q.split('&').filter_map(|p| p.split_once('=')).collect())
                            .unwrap_or_default();
                        let sensor = params.get("sensor").copied();
                        let since = params.get("since").map(|v| v.parse::<i64>()).transpose()?;
                        Ok(serde_json::to_value(store.snapshot_since(
                            sensor,
                            if demo { "demo" } else { "collector" },
                            since,
                        )?)?)
                    }
                    ("POST", "/v1/rename") | ("POST", "/v1/acknowledge") => {
                        if req.body_length().unwrap_or(4097) > 4096 {
                            bail!("Request exceeds 4 KiB");
                        }
                        let mut body = String::new();
                        std::io::Read::take(req.as_reader(), 4097).read_to_string(&mut body)?;
                        if body.len() > 4096 {
                            bail!("Request exceeds 4 KiB");
                        }
                        let value: serde_json::Value = serde_json::from_str(&body)?;
                        let id = value["id"].as_str().context("Missing id")?;
                        if req.url().split('?').next() == Some("/v1/rename") {
                            store.rename(id, value["name"].as_str().context("Missing name")?)?;
                        } else {
                            store.acknowledge(id)?;
                        }
                        Ok(serde_json::json!({"ok":true}))
                    }
                    _ => bail!("Unknown endpoint"),
                }
            })();
            match result {
                Ok(v) => (200, v),
                Err(e) => (400, serde_json::json!({"error":e.to_string()})),
            }
        };
        let response = Response::from_string(body.to_string())
            .with_status_code(StatusCode(status))
            .with_header(Header::from_bytes("Content-Type", "application/json").unwrap())
            .with_header(Header::from_bytes("Cache-Control", "no-store").unwrap());
        let _ = req.respond(response);
    }
    Ok(())
}
