//! QUIC Chat Server
//!
//! This application demonstrates a high-performance chat system using QUIC protocol
//! with multi-stream architecture for ultra-low-latency messaging.
//!
//! Usage:
//!   cargo run -- server                            # Run with defaults
//!   cargo run -- server --config config.toml       # Load config from file
//!   cargo run -- server --port 4433                # Override port

use garou::MultiStreamServer;
use garou::config::Config;
use std::env;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

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
            return Ok(());
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

/// Load config: from file if `--config` is specified, otherwise defaults.
fn load_config(args: &[String]) -> Result<Config, Box<dyn std::error::Error>> {
    for i in 0..args.len() {
        if args[i] == "--config" && i + 1 < args.len() {
            let path = &args[i + 1];
            let cfg = Config::load(path)?;
            return Ok(cfg);
        }
    }
    Ok(Config::default())
}

/// Apply CLI overrides onto an already-loaded Config.
fn apply_cli_overrides(config: &mut Config, args: &[String]) {
    for i in 0..args.len() {
        if args[i] == "--port" && i + 1 < args.len() {
            if let Ok(port) = args[i + 1].parse::<u16>() {
                // Replace port in bind_addr (keep host part)
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
    info!("Starting Multi-Stream QUIC Chat Server...");

    info!("Configuration:");
    info!("  - Bind address: {}", config.server.bind_addr);
    info!("  - Max connections: {}", config.server.max_connections);
    info!("  - Number of shards: {}", config.shard.num_shards);
    info!(
        "  - Hot room threshold: {} msgs/sec",
        config.shard.hot_room_threshold
    );
    info!("  - Datagrams enabled: {}", config.server.enable_datagrams);

    let server = MultiStreamServer::from_config(&config)?;

    if let Err(e) = server.start().await {
        error!("Server error: {}", e);
        return Err(e.into());
    }

    Ok(())
}
