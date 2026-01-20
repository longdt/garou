//! QUIC Chat Server and Client Example
//!
//! This application demonstrates a high-performance chat system using QUIC protocol
//! for communication and FlatBuffers for serialization.
//!
//! Usage:
//!   cargo run -- server                    # Run server
//!   cargo run -- client <username>         # Run client with username
//!   cargo run -- demo                      # Run demo with server and client

use garou::client::ClientEvent;
use garou::{ChatClient, ChatClientConfig, ChatConfig, ChatServer};
use std::env;
use std::time::Duration;
use tokio::time::{sleep, timeout};
use tracing::{error, info, warn};
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt().init();

    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage();
        return Ok(());
    }

    match args[1].as_str() {
        "server" => run_server().await?,
        "client" => {
            if args.len() < 3 {
                eprintln!("Usage: cargo run -- client <username>");
                return Ok(());
            }
            run_client(&args[2]).await?;
        }
        "demo" => run_demo().await?,
        "test" => run_test().await?,
        _ => {
            print_usage();
            return Ok(());
        }
    }

    Ok(())
}

fn print_usage() {
    println!("QUIC Chat Application");
    println!();
    println!("USAGE:");
    println!("    cargo run -- server");
    println!("    cargo run -- client <username>");
    println!("    cargo run -- demo");
    println!("    cargo run -- test");
    println!();
    println!("COMMANDS:");
    println!("    server              Start the chat server");
    println!("    client <username>   Connect as a client with the given username");
    println!("    demo               Run a demonstration with server and multiple clients");
    println!("    test               Run basic functionality tests");
}

async fn run_server() -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting QUIC Chat Server...");

    let config = ChatConfig {
        bind_addr: "127.0.0.1:4433".parse()?,
        max_connections: 100,
        idle_timeout_secs: 300,
        max_message_size: 1024 * 1024,
    };

    let mut server = ChatServer::new(config);

    // Start server (this will run indefinitely)
    if let Err(e) = server.start().await {
        error!("Server error: {}", e);
        return Err(e.into());
    }

    Ok(())
}

async fn run_client(username: &str) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting QUIC Chat Client for user: {}", username);

    let config = ChatClientConfig {
        server_addr: "127.0.0.1:4433".parse()?,
        bind_addr: "0.0.0.0:0".parse()?,
        connect_timeout_secs: 10,
        keep_alive_secs: 30,
        max_message_size: 1024 * 1024,
    };

    let mut client = ChatClient::new(config);

    // Connect to server
    let mut event_rx = match client.connect(username.to_string()).await {
        Ok(rx) => rx,
        Err(e) => {
            error!("Failed to connect: {}", e);
            return Err(e.into());
        }
    };

    info!("Connected to server as '{}'", username);
    info!("Type messages and press Enter to send. Type 'quit' to exit.");

    // Spawn task to read user input
    let client_for_input = std::sync::Arc::new(tokio::sync::Mutex::new(client));
    let input_client = client_for_input.clone();

    tokio::spawn(async move {
        let mut stdin = tokio::io::stdin();
        let mut buffer = String::new();

        loop {
            use tokio::io::AsyncBufReadExt;
            let mut reader = tokio::io::BufReader::new(&mut stdin);

            if let Ok(_) = reader.read_line(&mut buffer).await {
                let message = buffer.trim();

                if message == "quit" {
                    break;
                }

                if !message.is_empty() {
                    let client = input_client.lock().await;
                    if let Err(e) = client.send_message(message.to_string()).await {
                        error!("Failed to send message: {}", e);
                    }
                }

                buffer.clear();
            }
        }
    });

    // Handle events
    while let Some(event) = event_rx.recv().await {
        match event {
            ClientEvent::Connected => {
                info!("✓ Connected to chat server");
            }
            ClientEvent::Disconnected(reason) => {
                warn!("✗ Disconnected: {}", reason);
                break;
            }
            ClientEvent::MessageReceived(message) => {
                if let Some(sender) = &message.sender {
                    match &message.message_type {
                        garou::ChatMessageType::Text { content } => {
                            println!("[{}]: {}", sender.username, content);
                        }
                        garou::ChatMessageType::Join { user } => {
                            println!(">>> {} joined the chat", user.username);
                        }
                        garou::ChatMessageType::Leave { username, .. } => {
                            println!("<<< {} left the chat", username);
                        }
                        garou::ChatMessageType::UserList { users } => {
                            let usernames: Vec<String> =
                                users.iter().map(|u| u.username.clone()).collect();
                            println!("Users online ({}): {}", users.len(), usernames.join(", "));
                        }
                        garou::ChatMessageType::Error { message, .. } => {
                            error!("Server error: {}", message);
                        }
                    }
                } else {
                    match &message.message_type {
                        garou::ChatMessageType::Join { user } => {
                            println!(">>> {} joined the chat", user.username);
                        }
                        garou::ChatMessageType::Leave { username, .. } => {
                            println!("<<< {} left the chat", username);
                        }
                        garou::ChatMessageType::Error { message, .. } => {
                            error!("System error: {}", message);
                        }
                        _ => {}
                    }
                }
            }
            ClientEvent::Error(e) => {
                error!("Client error: {}", e);
            }
        }
    }

    // Disconnect
    let mut client = client_for_input.lock().await;
    if let Err(e) = client.disconnect().await {
        error!("Error during disconnect: {}", e);
    }

    info!("Chat client terminated");
    Ok(())
}

async fn run_demo() -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting QUIC Chat Demo...");

    // Start server in background
    let server_task = tokio::spawn(async {
        if let Err(e) = run_server().await {
            error!("Server task failed: {}", e);
        }
    });

    // Wait for server to start
    sleep(Duration::from_secs(2)).await;

    // Create multiple clients
    let client_tasks = vec![
        tokio::spawn(async {
            run_demo_client("Alice")
                .await
                .unwrap_or_else(|e| error!("Alice error: {}", e))
        }),
        tokio::spawn(async {
            run_demo_client("Bob")
                .await
                .unwrap_or_else(|e| error!("Bob error: {}", e))
        }),
        tokio::spawn(async {
            run_demo_client("Charlie")
                .await
                .unwrap_or_else(|e| error!("Charlie error: {}", e))
        }),
    ];

    // Wait for demo clients to complete
    for task in client_tasks {
        if let Err(e) = task.await {
            error!("Client task failed: {:?}", e);
        }
    }

    // Cancel server task
    server_task.abort();

    info!("Demo completed");
    Ok(())
}

async fn run_test() -> Result<(), Box<dyn std::error::Error>> {
    info!("Running basic functionality tests...");

    // Run the basic tests
    if let Err(e) = garou::simple_test::run_basic_test().await {
        error!("Basic test failed: {}", e);
        return Err(format!("Basic test failed: {}", e).into());
    }

    if let Err(e) = garou::simple_test::test_message_workflow().await {
        error!("Workflow test failed: {}", e);
        return Err(format!("Workflow test failed: {}", e).into());
    }

    if let Err(e) = garou::simple_test::test_serialization_performance().await {
        error!("Performance test failed: {}", e);
        return Err(format!("Performance test failed: {}", e).into());
    }

    info!("All tests passed successfully!");
    Ok(())
}

async fn run_demo_client(username: &str) -> Result<(), Box<dyn std::error::Error>> {
    sleep(Duration::from_millis(500)).await;

    let config = ChatClientConfig {
        server_addr: "127.0.0.1:4433".parse()?,
        bind_addr: "0.0.0.0:0".parse()?,
        connect_timeout_secs: 10,
        keep_alive_secs: 30,
        max_message_size: 1024 * 1024,
    };

    let mut client = ChatClient::new(config);

    let mut event_rx = timeout(
        Duration::from_secs(15),
        client.connect(username.to_string()),
    )
    .await??;

    info!("{} connected to demo chat", username);

    // Send a few messages
    sleep(Duration::from_secs(1)).await;
    client
        .send_message(format!("Hello from {}!", username))
        .await?;

    sleep(Duration::from_secs(2)).await;
    client
        .send_message(format!("This is {} speaking", username))
        .await?;

    if username == "Alice" {
        sleep(Duration::from_secs(1)).await;
        client.request_user_list().await?;
    }

    // Listen for messages for a while
    let start_time = std::time::Instant::now();
    while start_time.elapsed() < Duration::from_secs(10) {
        if let Ok(event) = timeout(Duration::from_millis(100), event_rx.recv()).await {
            if let Some(event) = event {
                match event {
                    ClientEvent::MessageReceived(message) => {
                        if let Some(sender) = &message.sender {
                            match &message.message_type {
                                garou::ChatMessageType::Text { content } => {
                                    info!("[{}] {}: {}", username, sender.username, content);
                                }
                                _ => {}
                            }
                        }
                    }
                    ClientEvent::Disconnected(_) => break,
                    ClientEvent::Error(_) => break,
                    _ => {}
                }
            }
        }
    }

    client.disconnect().await?;
    info!("{} disconnected from demo chat", username);

    Ok(())
}
