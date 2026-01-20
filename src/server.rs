//! QUIC-based chat server implementation
//!
//! This module provides the main chat server that handles client connections,
//! message routing, and user management using the QUIC protocol.

use crate::error::{ChatError, Result};
use crate::{ChatConfig, ChatMessage, ChatMessageType, User};
use quinn::{Connection, Endpoint};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};
use tracing::{debug, error, info, warn};

/// Maximum number of concurrent streams per connection
const MAX_CONCURRENT_STREAMS: u32 = 100;

/// Connection information for a connected client
#[derive(Debug, Clone)]
pub struct ClientConnection {
    pub user: User,
    pub connection: Connection,
    pub addr: SocketAddr,
    pub connected_at: u64,
}

/// Chat server state shared between handlers
#[derive(Debug)]
pub struct ServerState {
    /// Connected clients indexed by user ID
    clients: RwLock<HashMap<String, ClientConnection>>,
    /// Broadcast channel for distributing messages to all clients
    message_tx: broadcast::Sender<ChatMessage>,
    /// Server configuration
    config: ChatConfig,
}

impl ServerState {
    fn new(config: ChatConfig) -> Self {
        let (message_tx, _) = broadcast::channel(1000);
        Self {
            clients: RwLock::new(HashMap::new()),
            message_tx,
            config,
        }
    }

    /// Add a new client connection
    async fn add_client(&self, client: ClientConnection) -> Result<()> {
        let user_id = client.user.id.clone();
        let username = client.user.username.clone();

        // Check connection limit
        {
            let clients = self.clients.read().await;
            if clients.len() >= self.config.max_connections {
                return Err(ChatError::resource_limit(format!(
                    "Maximum connections reached: {}",
                    self.config.max_connections
                )));
            }
        }

        // Add client to the map
        self.clients.write().await.insert(user_id, client.clone());

        // Broadcast join message
        let join_message = ChatMessage::new_join(client.user.clone());
        let _ = self.message_tx.send(join_message);

        info!("User '{}' joined from {}", username, client.addr);

        Ok(())
    }

    /// Remove a client connection
    async fn remove_client(&self, user_id: &str) -> Result<()> {
        let client = self.clients.write().await.remove(user_id);

        if let Some(client) = client {
            // Broadcast leave message
            let leave_message =
                ChatMessage::new_leave(client.user.id.clone(), client.user.username.clone());
            let _ = self.message_tx.send(leave_message);

            info!("User '{}' left", client.user.username);
        }

        Ok(())
    }

    /// Broadcast a message to all connected clients
    async fn broadcast_message(&self, message: ChatMessage) -> Result<()> {
        self.message_tx
            .send(message)
            .map_err(|e| ChatError::internal(format!("Failed to broadcast message: {}", e)))?;
        Ok(())
    }

    /// Get list of all connected users
    async fn get_users(&self) -> Vec<User> {
        self.clients
            .read()
            .await
            .values()
            .map(|client| client.user.clone())
            .collect()
    }

    /// Get client count
    async fn client_count(&self) -> usize {
        self.clients.read().await.len()
    }
}

/// QUIC-based chat server
pub struct ChatServer {
    state: Arc<ServerState>,
    endpoint: Option<Endpoint>,
}

impl ChatServer {
    /// Create a new chat server with the given configuration
    pub fn new(config: ChatConfig) -> Self {
        Self {
            state: Arc::new(ServerState::new(config)),
            endpoint: None,
        }
    }

    /// Start the chat server
    pub async fn start(&mut self) -> Result<()> {
        info!("Starting chat server on {}", self.state.config.bind_addr);

        // Generate self-signed certificate for development
        let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()])
            .map_err(|e| ChatError::config(format!("Failed to generate certificate: {}", e)))?;

        let cert_der = CertificateDer::from(cert.serialize_der().unwrap());
        let key_der =
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(cert.serialize_private_key_der()));

        // Configure rustls
        let mut server_config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![cert_der], key_der)
            .map_err(|e| ChatError::config(format!("Failed to configure TLS: {}", e)))?;

        server_config.alpn_protocols = vec![b"h3".to_vec()];
        server_config.max_early_data_size = 0;

        // Configure QUIC
        let quic_config = quinn::ServerConfig::with_crypto(Arc::new(
            quinn::crypto::rustls::QuicServerConfig::try_from(server_config)
                .map_err(|e| ChatError::config(format!("Failed to create QUIC config: {}", e)))?,
        ));

        // Create endpoint
        let endpoint = Endpoint::server(quic_config, self.state.config.bind_addr)
            .map_err(|e| ChatError::network(format!("Failed to create endpoint: {}", e)))?;

        info!("Chat server listening on {}", endpoint.local_addr()?);

        self.endpoint = Some(endpoint.clone());

        // Accept connections
        self.accept_connections(endpoint).await
    }

    /// Accept and handle incoming connections
    async fn accept_connections(&self, endpoint: Endpoint) -> Result<()> {
        loop {
            match endpoint.accept().await {
                Some(incoming) => {
                    let state = Arc::clone(&self.state);
                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_connection(state, incoming).await {
                            error!("Connection handling failed: {}", e);
                        }
                    });
                }
                None => {
                    warn!("Endpoint stopped accepting connections");
                    break;
                }
            }
        }
        Ok(())
    }

    /// Handle a single client connection
    async fn handle_connection(state: Arc<ServerState>, incoming: quinn::Incoming) -> Result<()> {
        let connection = incoming.await?;
        let remote_addr = connection.remote_address();

        debug!("New connection from {}", remote_addr);

        // Wait for the client to open a bidirectional stream for authentication
        let (_send_stream, recv_stream) = match connection.accept_bi().await {
            Ok(streams) => streams,
            Err(e) => {
                error!("Failed to accept bidirectional stream: {}", e);
                return Err(ChatError::connection(format!(
                    "Failed to accept stream: {}",
                    e
                )));
            }
        };

        // Handle authentication (simplified for this example)
        let user = Self::authenticate_user(recv_stream).await?;

        // Create client connection
        let client = ClientConnection {
            user: user.clone(),
            connection: connection.clone(),
            addr: remote_addr,
            connected_at: crate::current_timestamp(),
        };

        // Add client to server state
        state.add_client(client).await?;
        let user_id = user.id.clone();

        // Subscribe to broadcast messages
        let mut message_rx = state.message_tx.subscribe();

        // Spawn task to send messages to this client
        let send_state = Arc::clone(&state);
        let send_connection = connection.clone();
        let send_user_id = user_id.clone();
        let send_task = tokio::spawn(async move {
            while let Ok(message) = message_rx.recv().await {
                // Don't echo messages back to sender
                if let Some(sender) = &message.sender {
                    if sender.id == send_user_id {
                        continue;
                    }
                }

                if let Err(e) = Self::send_message_to_client(&send_connection, &message).await {
                    error!("Failed to send message to client: {}", e);
                    break;
                }
            }

            // Clean up client on send task exit
            if let Err(e) = send_state.remove_client(&send_user_id).await {
                error!("Failed to remove client: {}", e);
            }
        });

        // Handle incoming messages from this client
        let recv_state = Arc::clone(&state);
        let recv_user_id = user_id.clone();
        let recv_user = user.clone();
        let recv_task = tokio::spawn(async move {
            if let Err(e) = Self::handle_client_messages(recv_state, connection, recv_user).await {
                error!("Client message handling failed: {}", e);
            }

            // Ensure client is removed when recv task exits
            if let Err(e) = state.remove_client(&recv_user_id).await {
                error!("Failed to remove client: {}", e);
            }
        });

        // Wait for either task to complete
        tokio::select! {
            _ = send_task => {},
            _ = recv_task => {},
        }

        info!("Connection closed for user: {}", user.username);
        Ok(())
    }

    /// Authenticate a user (simplified implementation)
    async fn authenticate_user(mut recv_stream: quinn::RecvStream) -> Result<User> {
        // Read authentication data
        let auth_data = recv_stream
            .read_to_end(256)
            .await
            .map_err(|e| ChatError::auth(format!("Failed to read auth data: {}", e)))?;

        // For this example, treat the auth data as username
        let username = String::from_utf8(auth_data)
            .map_err(|e| ChatError::auth(format!("Invalid username: {}", e)))?
            .trim()
            .to_string();

        if username.is_empty() || username.len() > 50 {
            return Err(ChatError::auth("Invalid username length"));
        }

        // Create user
        Ok(User::new(username))
    }

    /// Handle incoming messages from a client
    async fn handle_client_messages(
        state: Arc<ServerState>,
        connection: Connection,
        user: User,
    ) -> Result<()> {
        loop {
            // Accept a new bidirectional stream for each message
            let (mut send_stream, mut recv_stream) = connection.accept_bi().await?;

            // Read message data
            let message_data = recv_stream
                .read_to_end(state.config.max_message_size)
                .await
                .map_err(|e| ChatError::network(format!("Failed to read message: {}", e)))?;

            // Validate message size
            if message_data.len() > state.config.max_message_size {
                return Err(ChatError::invalid_message(format!(
                    "Message too large: {} bytes (max: {} bytes)",
                    message_data.len(),
                    state.config.max_message_size
                )));
            }

            // Parse message
            let mut message = serde_json::from_slice::<ChatMessage>(&message_data)
                .map_err(|e| ChatError::serialization(format!("Failed to parse message: {}", e)))?;

            // Set sender if not already set
            if message.sender.is_none() {
                message.sender = Some(user.clone());
            }

            // Process message based on type
            match &message.message_type {
                ChatMessageType::Text { content } => {
                    info!("Message from {}: {}", user.username, content);
                    state.broadcast_message(message).await?;
                }
                ChatMessageType::UserList { .. } => {
                    // Send current user list back to requesting client
                    let users = state.get_users().await;
                    let user_list_message = ChatMessage::new_user_list(users);

                    let response_data = serde_json::to_vec(&user_list_message).map_err(|e| {
                        ChatError::serialization(format!("Failed to serialize response: {}", e))
                    })?;

                    send_stream.write_all(&response_data).await?;
                    send_stream
                        .finish()
                        .map_err(|_| ChatError::connection("Failed to close response stream"))?;
                }
                _ => {
                    // Broadcast other message types
                    state.broadcast_message(message).await?;
                }
            }
        }
    }

    /// Send a message to a specific client
    async fn send_message_to_client(connection: &Connection, message: &ChatMessage) -> Result<()> {
        // Serialize message
        let message_data = serde_json::to_vec(message)
            .map_err(|e| ChatError::serialization(format!("Failed to serialize message: {}", e)))?;

        // Open a new stream for this message
        let mut send_stream = connection.open_uni().await?;

        // Send message
        send_stream.write_all(&message_data).await?;
        send_stream
            .finish()
            .map_err(|_| ChatError::connection("Failed to close message stream"))?;

        Ok(())
    }

    /// Get server statistics
    pub async fn get_stats(&self) -> ServerStats {
        let client_count = self.state.client_count().await;
        ServerStats {
            connected_clients: client_count,
            bind_address: self.state.config.bind_addr,
            max_connections: self.state.config.max_connections,
        }
    }

    /// Gracefully shutdown the server
    pub async fn shutdown(&mut self) -> Result<()> {
        if let Some(endpoint) = self.endpoint.take() {
            endpoint.close(0u32.into(), b"Server shutdown");
            info!("Chat server shutdown completed");
        }
        Ok(())
    }
}

/// Server statistics
#[derive(Debug, Clone)]
pub struct ServerStats {
    pub connected_clients: usize,
    pub bind_address: SocketAddr,
    pub max_connections: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ChatConfig;

    #[tokio::test]
    async fn test_server_creation() {
        let config = ChatConfig::default();
        let server = ChatServer::new(config.clone());

        assert_eq!(server.state.config.bind_addr, config.bind_addr);
        assert_eq!(server.state.config.max_connections, config.max_connections);
    }

    #[tokio::test]
    async fn test_state_operations() {
        let config = ChatConfig::default();
        let state = ServerState::new(config);

        // Test initial state
        assert_eq!(state.client_count().await, 0);
        let users = state.get_users().await;
        assert_eq!(users.len(), 0);
    }
}
