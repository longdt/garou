//! Simple test to verify basic QUIC chat functionality
//!
//! This module provides a minimal test to ensure the core components work correctly.

use crate::{
    ChatClient, ChatClientConfig, ChatConfig, ChatMessage, ChatMessageType, ChatServer, User,
};
use std::time::Duration;
use tokio::time::timeout;
use tracing::{error, info};

/// Run a basic functionality test
pub async fn run_basic_test() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    info!("Starting basic QUIC chat test...");

    // Test 1: Message serialization/deserialization
    test_message_serialization().await?;

    // Test 2: User creation
    test_user_creation().await?;

    // Test 3: Configuration
    test_configuration().await?;

    info!("Basic test completed successfully!");
    Ok(())
}

async fn test_message_serialization() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    info!("Testing message serialization...");

    let user = User::new("testuser".to_string());
    let message = ChatMessage::new_text(user.clone(), "Hello, world!".to_string());

    // Serialize to JSON
    let serialized = serde_json::to_vec(&message)?;

    // Deserialize back
    let deserialized: ChatMessage = serde_json::from_slice(&serialized)?;

    // Verify data integrity
    assert_eq!(message.id, deserialized.id);
    assert_eq!(message.sender, deserialized.sender);
    assert_eq!(message.timestamp, deserialized.timestamp);

    match (&message.message_type, &deserialized.message_type) {
        (ChatMessageType::Text { content: orig }, ChatMessageType::Text { content: deser }) => {
            assert_eq!(orig, deser);
        }
        _ => return Err("Message type mismatch".into()),
    }

    info!("✓ Message serialization test passed");
    Ok(())
}

async fn test_user_creation() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    info!("Testing user creation...");

    let user = User::new("alice".to_string());

    assert!(!user.id.is_empty());
    assert_eq!(user.username, "alice");
    assert!(user.joined_at > 0);

    // Test different message types
    let text_msg = ChatMessage::new_text(user.clone(), "Hello!".to_string());
    let join_msg = ChatMessage::new_join(user.clone());
    let leave_msg = ChatMessage::new_leave(user.id.clone(), user.username.clone());
    let users_msg = ChatMessage::new_user_list(vec![user.clone()]);
    let error_msg = ChatMessage::new_error(404, "Not found".to_string());

    assert!(matches!(
        text_msg.message_type,
        ChatMessageType::Text { .. }
    ));
    assert!(matches!(
        join_msg.message_type,
        ChatMessageType::Join { .. }
    ));
    assert!(matches!(
        leave_msg.message_type,
        ChatMessageType::Leave { .. }
    ));
    assert!(matches!(
        users_msg.message_type,
        ChatMessageType::UserList { .. }
    ));
    assert!(matches!(
        error_msg.message_type,
        ChatMessageType::Error { .. }
    ));

    info!("✓ User creation test passed");
    Ok(())
}

async fn test_configuration() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    info!("Testing configuration...");

    // Test server config
    let server_config = ChatConfig {
        bind_addr: "127.0.0.1:4434".parse()?,
        max_connections: 50,
        idle_timeout_secs: 120,
        max_message_size: 512 * 1024,
        ..Default::default()
    };

    assert_eq!(server_config.bind_addr.port(), 4434);
    assert_eq!(server_config.max_connections, 50);

    // Test client config
    let client_config = ChatClientConfig {
        server_addr: "127.0.0.1:4434".parse()?,
        bind_addr: "0.0.0.0:0".parse()?,
        connect_timeout_secs: 5,
        keep_alive_secs: 30,
        max_message_size: 512 * 1024,
    };

    assert_eq!(client_config.server_addr.port(), 4434);
    assert_eq!(client_config.connect_timeout_secs, 5);

    info!("✓ Configuration test passed");
    Ok(())
}

/// Test that demonstrates the basic workflow without actual network connections
pub async fn test_message_workflow() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    info!("Testing message workflow...");

    // Create users
    let alice = User::new("Alice".to_string());
    let bob = User::new("Bob".to_string());

    // Simulate a conversation
    let messages = vec![
        ChatMessage::new_join(alice.clone()),
        ChatMessage::new_text(alice.clone(), "Hello everyone!".to_string()),
        ChatMessage::new_join(bob.clone()),
        ChatMessage::new_text(bob.clone(), "Hi Alice!".to_string()),
        ChatMessage::new_text(alice.clone(), "How are you doing, Bob?".to_string()),
        ChatMessage::new_user_list(vec![alice.clone(), bob.clone()]),
        ChatMessage::new_leave(alice.id.clone(), alice.username.clone()),
        ChatMessage::new_text(bob.clone(), "Goodbye Alice!".to_string()),
    ];

    // Test that all messages can be serialized and deserialized
    for message in messages {
        let serialized = serde_json::to_vec(&message)?;
        let _deserialized: ChatMessage = serde_json::from_slice(&serialized)?;

        info!("Message: {:?}", message.message_type);
    }

    info!("✓ Message workflow test passed");
    Ok(())
}

/// Performance test for message serialization
pub async fn test_serialization_performance() -> Result<(), Box<dyn std::error::Error + Send + Sync>>
{
    info!("Testing serialization performance...");

    let user = User::new("performance_test_user".to_string());
    let message = ChatMessage::new_text(
        user,
        "This is a performance test message with some content to serialize.".to_string(),
    );

    let start = std::time::Instant::now();
    let iterations = 10000;

    for _ in 0..iterations {
        let serialized = serde_json::to_vec(&message)?;
        let _deserialized: ChatMessage = serde_json::from_slice(&serialized)?;
    }

    let elapsed = start.elapsed();
    let rate = iterations as f64 / elapsed.as_secs_f64();

    info!(
        "✓ Serialization performance: {:.0} ops/sec ({} iterations in {:?})",
        rate, iterations, elapsed
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_basic_functionality() {
        run_basic_test().await.expect("Basic test should pass");
    }

    #[tokio::test]
    async fn test_workflow() {
        test_message_workflow()
            .await
            .expect("Workflow test should pass");
    }

    #[tokio::test]
    async fn test_perf() {
        test_serialization_performance()
            .await
            .expect("Performance test should pass");
    }

    #[test]
    fn test_message_types() {
        let user = User::new("test".to_string());

        // Test all message type variants
        let text = ChatMessage::new_text(user.clone(), "test".to_string());
        let join = ChatMessage::new_join(user.clone());
        let leave = ChatMessage::new_leave(user.id.clone(), user.username.clone());
        let users = ChatMessage::new_user_list(vec![user.clone()]);
        let error = ChatMessage::new_error(500, "test error".to_string());

        assert!(matches!(text.message_type, ChatMessageType::Text { .. }));
        assert!(matches!(join.message_type, ChatMessageType::Join { .. }));
        assert!(matches!(leave.message_type, ChatMessageType::Leave { .. }));
        assert!(matches!(
            users.message_type,
            ChatMessageType::UserList { .. }
        ));
        assert!(matches!(error.message_type, ChatMessageType::Error { .. }));

        // Ensure all messages have unique IDs
        let ids = vec![&text.id, &join.id, &leave.id, &users.id, &error.id];
        let unique_ids: std::collections::HashSet<_> = ids.into_iter().collect();
        assert_eq!(unique_ids.len(), 5);
    }

    #[test]
    fn test_user_uniqueness() {
        let user1 = User::new("same_name".to_string());
        let user2 = User::new("same_name".to_string());

        // Same username but different IDs
        assert_eq!(user1.username, user2.username);
        assert_ne!(user1.id, user2.id);
        assert!(user1.joined_at <= user2.joined_at); // user2 created after user1
    }
}
