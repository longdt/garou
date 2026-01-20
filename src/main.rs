//! QUIC Chat Server
//!
//! This application demonstrates a high-performance chat system using QUIC protocol
//! with multi-stream architecture for ultra-low-latency messaging.
//!
//! Usage:
//!   cargo run -- server                    # Run multi-stream server
//!   cargo run -- server --port 4433        # Run on specific port

use garou::MultiStreamServer;
use garou::server::multi_stream_server::ServerConfig;
use std::env;
use std::time::Duration;
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
            let port = parse_port(&args);
            run_server(port).await?;
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
    println!("    --port <PORT>       Port to listen on (default: 4433)");
    println!("    --max-conn <NUM>    Maximum connections (default: 10000)");
    println!();
    println!("MULTI-STREAM ARCHITECTURE:");
    println!("    The server uses separate QUIC streams for different message types:");
    println!("    - Control Stream (bidirectional): Auth, ping/pong, commands");
    println!("    - Chat Commands Stream (client→server): Messages, reactions, edits");
    println!("    - ACK Stream (client→server): Delivery/read receipts");
    println!("    - Shard Streams (server→client): Room messages grouped by shard");
    println!("    - Hot Room Streams (server→client): Dedicated streams for high-traffic rooms");
    println!("    - Datagrams: Typing indicators, presence (unreliable)");
    println!();
    println!("EXAMPLES:");
    println!("    cargo run -- server");
    println!("    cargo run -- server --port 5000");
    println!("    RUST_LOG=debug cargo run -- server");
}

fn parse_port(args: &[String]) -> u16 {
    for i in 0..args.len() {
        if args[i] == "--port" && i + 1 < args.len() {
            if let Ok(port) = args[i + 1].parse() {
                return port;
            }
        }
    }
    4433 // default port
}

fn parse_max_connections(args: &[String]) -> usize {
    for i in 0..args.len() {
        if args[i] == "--max-conn" && i + 1 < args.len() {
            if let Ok(max) = args[i + 1].parse() {
                return max;
            }
        }
    }
    10000 // default
}

async fn run_server(port: u16) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting Multi-Stream QUIC Chat Server...");
    info!(
        "Architecture: Separate streams for control, chat commands, ACKs, and sharded room messages"
    );

    let args: Vec<String> = env::args().collect();
    let max_connections = parse_max_connections(&args);

    let config = ServerConfig {
        bind_addr: format!("0.0.0.0:{}", port).parse()?,
        max_connections,
        idle_timeout: Duration::from_secs(300),
        enable_datagrams: true,
        ..Default::default()
    };

    info!("Configuration:");
    info!("  - Bind address: {}", config.bind_addr);
    info!("  - Max connections: {}", config.max_connections);
    info!("  - Number of shards: {}", config.shard_config.num_shards);
    info!(
        "  - Hot room threshold: {} msgs/sec",
        config.shard_config.hot_room_threshold
    );
    info!("  - Datagrams enabled: {}", config.enable_datagrams);

    let mut server = MultiStreamServer::new(config);

    // Start server (this will run indefinitely)
    if let Err(e) = server.start().await {
        error!("Server error: {}", e);
        return Err(e.into());
    }

    Ok(())
}
