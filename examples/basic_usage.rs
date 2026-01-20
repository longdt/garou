//! Basic usage example for the QUIC Chat Server
//!
//! This example demonstrates how to programmatically use the chat server
//! and client APIs for building chat applications.

use garou::client::{ChatClientConfig, ClientEvent};
use garou::{ChatClient, ChatConfig, ChatMessage, ChatMessageType, ChatServer};

use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize rustls crypto provider
    let _ = rustls::crypto::ring::default_provider().install_default();

    // Initialize logging
    tracing_subscriber::fmt().init();

    info!("Starting QUIC Chat Basic Usage Example");

    // Example 1: Start a server programmatically
    tokio::spawn(async move {
        if let Err(e) = run_example_server().await {
            error!("Server error: {}", e);
        }
    });

    // Wait for server to start
    sleep(Duration::from_secs(1)).await;

    // Example 2: Create clients and demonstrate messaging
    let client_handles = vec![
        tokio::spawn(async move {
            if let Err(e) = run_example_client(
                "Alice",
                vec![
                    "Hello everyone!".to_string(),
                    "How is everyone doing?".to_string(),
                ],
            )
            .await
            {
                error!("Alice client error: {}", e);
            }
        }),
        tokio::spawn(async move {
            if let Err(e) = run_example_client(
                "Bob",
                vec![
                    "Hi Alice!".to_string(),
                    "I'm doing great, thanks!".to_string(),
                ],
            )
            .await
            {
                error!("Bob client error: {}", e);
            }
        }),
        tokio::spawn(async move {
            if let Err(e) = run_example_client(
                "Charlie",
                vec![
                    "Hey there!".to_string(),
                    "This QUIC chat is really fast!".to_string(),
                ],
            )
            .await
            {
                error!("Charlie client error: {}", e);
            }
        }),
    ];

    // Wait for all clients to complete
    for handle in client_handles {
        if let Err(e) = handle.await {
            error!("Client join error: {:?}", e);
        }
    }

    info!("Basic usage example completed");
    Ok(())
}

async fn run_example_server() -> Result<(), Box<dyn std::error::Error>> {
    let config = ChatConfig {
        bind_addr: "127.0.0.1:4433".parse()?,
        max_connections: 50,
        idle_timeout_secs: 60,
        max_message_size: 512 * 1024, // 512KB
    };

    let mut server = ChatServer::new(config);

    info!("📡 Example server starting on {}", "127.0.0.1:4433");

    // Run server for the duration of the example
    let result = tokio::time::timeout(Duration::from_secs(15), server.start()).await;

    match result {
        Ok(Err(e)) => error!("Server error: {}", e),
        Err(_) => info!("Server timed out (expected for this example)"),
        _ => {}
    }

    Ok(())
}

async fn run_example_client(
    username: &str,
    messages_to_send: Vec<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Small delay to stagger client connections
    sleep(Duration::from_millis(200)).await;

    let config = ChatClientConfig {
        server_addr: "127.0.0.1:4433".parse()?,
        bind_addr: "0.0.0.0:0".parse()?,
        connect_timeout_secs: 5,
        keep_alive_secs: 20,
        max_message_size: 512 * 1024,
    };

    let mut client = ChatClient::new(config);

    // Connect to server
    let mut event_rx = client.connect(username.to_string()).await?;
    info!("🚀 {} connected to chat", username);

    // Check connection status
    if !client.is_connected() {
        error!("❌ {} connection check failed", username);
        return Err("Connection failed".into());
    }
    info!("✅ {} connection verified", username);

    // Handle events in a separate task
    let username_for_events = username.to_string();
    let event_handle = tokio::spawn(async move {
        let mut message_count = 0;
        info!("🔄 {} started event handling loop", username_for_events);
        while let Some(event) = event_rx.recv().await {
            info!("📥 {} received event: {:?}", username_for_events, event);
            match event {
                ClientEvent::Connected => {
                    info!("✅ {} successfully connected", username_for_events);
                }
                ClientEvent::Disconnected(reason) => {
                    info!("❌ {} disconnected: {}", username_for_events, reason);
                    break;
                }
                ClientEvent::MessageReceived(message) => {
                    handle_received_message(&username_for_events, &message);
                    message_count += 1;

                    // Stop after receiving a reasonable number of messages
                    if message_count >= 10 {
                        break;
                    }
                }
                ClientEvent::Error(e) => {
                    error!("❗ {} error: {}", username_for_events, e);
                }
            }
        }
        info!("🔚 {} event handling loop ended", username_for_events);
    });

    // Send messages with delays
    for (i, message) in messages_to_send.iter().enumerate() {
        let delay_ms = 1000 + (i as u64 * 500);
        info!(
            "⏱️  {} waiting {}ms before sending message",
            username, delay_ms
        );
        sleep(Duration::from_millis(delay_ms)).await;

        info!("🔄 {} attempting to send message: {}", username, message);

        // Check if still connected before sending
        if !client.is_connected() {
            error!("❌ {} not connected, cannot send message", username);
            break;
        }

        if let Err(e) = client.send_message(message.clone()).await {
            error!("Failed to send message from {}: {}", username, e);
        } else {
            info!("📤 {} sent: {}", username, message);
        }
    }

    // Request user list
    sleep(Duration::from_millis(500)).await;
    info!("👥 {} requesting user list", username);

    // Check if still connected before requesting user list
    if !client.is_connected() {
        error!("❌ {} not connected, cannot request user list", username);
    } else if let Err(e) = client.request_user_list().await {
        error!("Failed to request user list: {}", e);
    } else {
        info!("✅ {} user list request sent", username);
    }

    // Wait a bit more to receive messages from others
    info!("⏱️  {} waiting 3 seconds to receive messages", username);
    sleep(Duration::from_secs(3)).await;

    // Clean up
    event_handle.abort();
    client.disconnect().await?;

    info!("👋 {} left the chat", username);
    Ok(())
}

fn handle_received_message(username: &str, message: &ChatMessage) {
    match &message.message_type {
        ChatMessageType::Text { content } => {
            if let Some(sender) = &message.sender {
                // Don't log our own messages
                if sender.username != username {
                    info!(
                        "📨 {} received from {}: {}",
                        username, sender.username, content
                    );
                }
            }
        }
        ChatMessageType::Join { user } => {
            if user.username != username {
                info!("👋 {} sees that {} joined", username, user.username);
            }
        }
        ChatMessageType::Leave {
            username: left_user,
            ..
        } => {
            if left_user != username {
                info!("👋 {} sees that {} left", username, left_user);
            }
        }
        ChatMessageType::UserList { users } => {
            let user_names: Vec<String> = users.iter().map(|u| u.username.clone()).collect();
            info!(
                "👥 {} sees {} users online: {}",
                username,
                users.len(),
                user_names.join(", ")
            );
        }
        ChatMessageType::Error {
            code,
            message: error_msg,
        } => {
            error!("❌ {} received error {}: {}", username, code, error_msg);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_example_client_creation() {
        let config = ChatClientConfig::default();
        let client = ChatClient::new(config);
        assert!(!client.is_connected());
    }

    #[test]
    fn test_server_config() {
        let config = ChatConfig {
            bind_addr: "127.0.0.1:4434".parse().unwrap(),
            max_connections: 10,
            idle_timeout_secs: 30,
            max_message_size: 1024,
        };

        assert_eq!(config.bind_addr.port(), 4434);
        assert_eq!(config.max_connections, 10);
        assert_eq!(config.idle_timeout_secs, 30);
        assert_eq!(config.max_message_size, 1024);
    }
}
