//! QUIC Chat Server
//!
//! Usage:
//!   cargo run -- server                            # Run with defaults
//!   cargo run -- server --config config.toml       # Load config from file
//!   cargo run -- server --port 4433                # Override port

use std::env;
use std::sync::Arc;

use garou::MultiStreamServer;
use garou::config::Config;
use garou::health::{HealthDeps, spawn_health_server};
use garou::metrics::{Metrics, init_telemetry};
use garou::shutdown::{ShutdownCoordinator, install_signal_handlers};
use opentelemetry::global;
use tracing::{error, info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage();
        return Ok(());
    }

    match args[1].as_str() {
        "server" => {
            let mut config = load_config(&args)?;
            apply_cli_overrides(&mut config, &args);
            run_server(config).await?;
        }
        "help" | "--help" | "-h" => {
            print_usage();
        }
        _ => {
            eprintln!("Unknown command: {}", args[1]);
            print_usage();
        }
    }

    Ok(())
}

fn print_usage() {
    println!("Garou - High-Performance QUIC Chat Server");
    println!();
    println!("USAGE:");
    println!("    cargo run -- server [OPTIONS]");
    println!();
    println!("COMMANDS:");
    println!("    server              Start the multi-stream chat server");
    println!("    help                Show this help message");
    println!();
    println!("OPTIONS:");
    println!("    --config <PATH>     Path to TOML config file");
    println!("    --port <PORT>       Override listen port (default: 4433)");
    println!("    --max-conn <NUM>    Override maximum connections (default: 10000)");
    println!();
    println!("MULTI-STREAM ARCHITECTURE:");
    println!("    The server uses separate QUIC streams for different message types:");
    println!("    - Control Stream (bidirectional): Auth, ping/pong, commands");
    println!("    - Chat Commands Stream (client->server): Messages, reactions, edits");
    println!("    - ACK Stream (client->server): Delivery/read receipts");
    println!("    - Shard Streams (server->client): Room messages grouped by shard");
    println!("    - Hot Room Streams (server->client): Dedicated streams for high-traffic rooms");
    println!("    - Datagrams: Typing indicators, presence (unreliable)");
    println!();
    println!("EXAMPLES:");
    println!("    cargo run -- server");
    println!("    cargo run -- server --config config.toml");
    println!("    cargo run -- server --port 5000");
    println!("    RUST_LOG=debug cargo run -- server");
}

fn load_config(args: &[String]) -> Result<Config, Box<dyn std::error::Error>> {
    for i in 0..args.len() {
        if args[i] == "--config" && i + 1 < args.len() {
            return Ok(Config::load(&args[i + 1])?);
        }
    }
    Ok(Config::default())
}

fn apply_cli_overrides(config: &mut Config, args: &[String]) {
    for i in 0..args.len() {
        if args[i] == "--port" && i + 1 < args.len() {
            if let Ok(port) = args[i + 1].parse::<u16>() {
                if let Some(colon) = config.server.bind_addr.rfind(':') {
                    config.server.bind_addr =
                        format!("{}:{}", &config.server.bind_addr[..colon], port);
                } else {
                    config.server.bind_addr = format!("0.0.0.0:{}", port);
                }
            }
        }
        if args[i] == "--max-conn" && i + 1 < args.len() {
            if let Ok(max) = args[i + 1].parse::<usize>() {
                config.server.max_connections = max;
            }
        }
    }
}

async fn run_server(config: Config) -> Result<(), Box<dyn std::error::Error>> {
    // ── 1. Telemetry ────────────────────────────────────────────────────────
    // Guard must live for the entire process lifetime so providers flush on drop.
    let _telemetry_guard = if config.observability.enabled {
        Some(init_telemetry(
            "garou",
            &config.observability.otlp_endpoint,
        ))
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .init();
        None
    };

    let metrics = Arc::new(Metrics::new(&global::meter("garou")));

    // ── 2. QUIC server (storage clients live inside) ─────────────────────────
    info!("Starting Multi-Stream QUIC Chat Server...");
    info!("  bind_addr         : {}", config.server.bind_addr);
    info!("  max_connections   : {}", config.server.max_connections);
    info!("  num_shards        : {}", config.shard.num_shards);
    info!("  hot_room_threshold: {} msgs/sec", config.shard.hot_room_threshold);
    info!("  datagrams         : {}", config.server.enable_datagrams);
    info!("  otlp_endpoint     : {}", config.observability.otlp_endpoint);

    let server = MultiStreamServer::from_config(&config).await?;

    // ── 3. Health probes — wired to real storage clients ─────────────────────
    let (nats_handle, redis_handle) = server.storage_handles();
    let health_deps = HealthDeps::new(nats_handle, redis_handle);
    let health_addr: std::net::SocketAddr = config.observability.health_addr.parse()?;
    spawn_health_server(health_addr, Arc::clone(&health_deps))?;
    info!("  health_addr       : {}", health_addr);

    // ── 4. Signal handling ───────────────────────────────────────────────────
    let coordinator = Arc::new(ShutdownCoordinator::new(
        config.server.shutdown_timeout_secs,
    ));
    let mut signal_rx = install_signal_handlers();

    let coord_bg = Arc::clone(&coordinator);
    let server_task = tokio::spawn(async move {
        if let Err(e) = server.start().await {
            error!("Server error: {e}");
        }
        coord_bg.initiate(); // ensure shutdown propagates if server exits first
    });

    // ── 5. Wait for signal ───────────────────────────────────────────────────
    tokio::select! {
        _ = signal_rx.recv() => {
            info!("Shutdown signal received — beginning graceful drain");
        }
        _ = server_task => {
            info!("Server task exited — beginning graceful drain");
        }
    }

    // ── 6. Ordered shutdown ──────────────────────────────────────────────────
    // Stop accepting new connections and mark health as not-ready.
    health_deps.set_accepting(false);
    coordinator.initiate();
    metrics.set_rooms_delta(0); // flush any pending gauge

    info!("Draining {} active connection(s)...", coordinator.active_count());
    coordinator.wait_drained().await;

    // Telemetry providers are flushed when _telemetry_guard drops at end of scope.
    info!("Shutdown complete");
    Ok(())
}
