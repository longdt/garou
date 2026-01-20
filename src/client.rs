//! QUIC-based chat client implementation
//!
//! This module provides a client for connecting to the QUIC chat server,
//! sending messages, and receiving real-time updates.

use crate::error::{ChatError, Result};
use crate::{ChatMessage, ChatMessageType, User};
use quinn::{ClientConfig as QuinnClientConfig, Connection, Endpoint};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, info};

/// Chat client configuration
#[derive(Clone, Debug)]
pub struct ChatClientConfig {
    /// Server address to connect to
    pub server_addr: SocketAddr,
    /// Client bind address (use 0.0.0.0:0 for auto)
    pub bind_addr: SocketAddr,
    /// Connection timeout in seconds
    pub connect_timeout_secs: u64,
    /// Keep-alive interval in seconds
    pub keep_alive_secs: u64,
    /// Maximum message size in bytes
    pub max_message_size: usize,
}

impl Default for ChatClientConfig {
    fn default() -> Self {
        Self {
            server_addr: "127.0.0.1:4433".parse().unwrap(),
            bind_addr: "0.0.0.0:0".parse().unwrap(),
            connect_timeout_secs: 10,
            keep_alive_secs: 30,
            max_message_size: 1024 * 1024, // 1MB
        }
    }
}

/// Events that the client can receive
#[derive(Debug, Clone)]
pub enum ClientEvent {
    /// Successfully connected to server
    Connected,
    /// Disconnected from server
    Disconnected(String),
    /// Received a chat message
    MessageReceived(ChatMessage),
    /// Error occurred
    Error(ChatError),
}

/// QUIC-based chat client
pub struct ChatClient {
    config: ChatClientConfig,
    user: Option<User>,
    connection: Option<Connection>,
    endpoint: Option<Endpoint>,
    event_tx: Option<mpsc::UnboundedSender<ClientEvent>>,
}

impl ChatClient {
    /// Create a new chat client with the given configuration
    pub fn new(config: ChatClientConfig) -> Self {
        Self {
            config,
            user: None,
            connection: None,
            endpoint: None,
            event_tx: None,
        }
    }

    /// Connect to the chat server with the given username
    pub async fn connect(
        &mut self,
        username: String,
    ) -> Result<mpsc::UnboundedReceiver<ClientEvent>> {
        info!("Connecting to chat server at {}", self.config.server_addr);

        // Create user
        let user = User::new(username);
        self.user = Some(user.clone());

        // Configure client
        let client_config = self.configure_client()?;

        // Create endpoint
        let mut endpoint = Endpoint::client(self.config.bind_addr)
            .map_err(|e| ChatError::network(format!("Failed to create endpoint: {}", e)))?;

        endpoint.set_default_client_config(client_config);
        self.endpoint = Some(endpoint.clone());

        // Connect to server
        let connecting = endpoint
            .connect(self.config.server_addr, "localhost")
            .map_err(|e| ChatError::connection(format!("Failed to initiate connection: {}", e)))?;

        let connection = tokio::time::timeout(
            std::time::Duration::from_secs(self.config.connect_timeout_secs),
            connecting,
        )
        .await
        .map_err(|_| ChatError::timeout("Connection timeout"))?
        .map_err(|e| ChatError::connection(format!("Failed to connect: {}", e)))?;

        info!("Successfully connected to server");
        self.connection = Some(connection.clone());

        // Authenticate with server
        self.authenticate(&connection, &user).await?;

        // Set up event channel
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        self.event_tx = Some(event_tx.clone());

        // Send connected event
        let _ = event_tx.send(ClientEvent::Connected);

        // Start message receiving task
        self.start_message_receiver(connection, event_tx).await?;

        Ok(event_rx)
    }

    /// Configure the QUIC client
    fn configure_client(&self) -> Result<QuinnClientConfig> {
        // Create a custom certificate verifier that accepts self-signed certificates
        // WARNING: This is insecure and should only be used for development/testing
        let mut crypto = rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(AcceptAnyCertificate))
            .with_no_client_auth();

        // Set ALPN protocol to match server
        crypto.alpn_protocols = vec![b"chat".to_vec()];

        Ok(QuinnClientConfig::new(Arc::new(
            quinn::crypto::rustls::QuicClientConfig::try_from(crypto)
                .map_err(|e| ChatError::config(format!("Failed to create QUIC config: {}", e)))?,
        )))
    }

    /// Authenticate with the server
    async fn authenticate(&self, connection: &Connection, user: &User) -> Result<()> {
        let (mut send_stream, _recv_stream) = connection.open_bi().await?;

        // Send username as authentication
        send_stream.write_all(user.username.as_bytes()).await?;
        send_stream
            .finish()
            .map_err(|_| ChatError::connection("Failed to close auth stream"))?;

        // Authentication is considered successful when the stream is sent
        // The server doesn't send a response back for this simple authentication
        info!("Authentication successful for user: {}", user.username);
        Ok(())
    }

    /// Start the message receiving task
    async fn start_message_receiver(
        &self,
        connection: Connection,
        event_tx: mpsc::UnboundedSender<ClientEvent>,
    ) -> Result<()> {
        let max_message_size = self.config.max_message_size;

        tokio::spawn(async move {
            loop {
                match connection.accept_uni().await {
                    Ok(mut recv_stream) => match recv_stream.read_to_end(max_message_size).await {
                        Ok(message_data) => {
                            match serde_json::from_slice::<ChatMessage>(&message_data) {
                                Ok(message) => {
                                    let _ = event_tx.send(ClientEvent::MessageReceived(message));
                                }
                                Err(e) => {
                                    error!("Failed to parse message: {}", e);
                                    let _ = event_tx.send(ClientEvent::Error(
                                        ChatError::serialization(format!(
                                            "Failed to parse message: {}",
                                            e
                                        )),
                                    ));
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to read message: {}", e);
                            let _ = event_tx.send(ClientEvent::Error(ChatError::network(format!(
                                "Failed to read message: {}",
                                e
                            ))));
                        }
                    },
                    Err(e) => {
                        error!("Failed to accept stream: {}", e);
                        let _ = event_tx
                            .send(ClientEvent::Disconnected(format!("Connection lost: {}", e)));
                        break;
                    }
                }
            }
        });

        Ok(())
    }

    /// Send a text message to the chat
    pub async fn send_message(&self, content: String) -> Result<()> {
        let connection = self
            .connection
            .as_ref()
            .ok_or_else(|| ChatError::connection("Not connected to server"))?;

        let user = self
            .user
            .as_ref()
            .ok_or_else(|| ChatError::internal("User not set"))?;

        // Create message
        let message = ChatMessage::new_text(user.clone(), content);

        // Serialize message
        let message_data = serde_json::to_vec(&message)
            .map_err(|e| ChatError::serialization(format!("Failed to serialize message: {}", e)))?;

        // Send message
        let (mut send_stream, mut recv_stream) = connection.open_bi().await?;
        send_stream.write_all(&message_data).await?;
        send_stream
            .finish()
            .map_err(|_| ChatError::connection("Failed to close message stream"))?;

        // Read response (if any)
        let _ = recv_stream.read_to_end(1024).await;

        debug!("Sent message: {}", message.id);
        Ok(())
    }

    /// Request the current user list
    pub async fn request_user_list(&self) -> Result<()> {
        let connection = self
            .connection
            .as_ref()
            .ok_or_else(|| ChatError::connection("Not connected to server"))?;

        let user = self
            .user
            .as_ref()
            .ok_or_else(|| ChatError::internal("User not set"))?;

        // Create user list request
        let message = ChatMessage {
            id: crate::generate_message_id(),
            sender: Some(user.clone()),
            message_type: ChatMessageType::UserList { users: Vec::new() },
            timestamp: crate::current_timestamp(),
        };

        // Serialize and send request
        let message_data = serde_json::to_vec(&message)
            .map_err(|e| ChatError::serialization(format!("Failed to serialize request: {}", e)))?;

        let (mut send_stream, mut recv_stream) = connection.open_bi().await?;
        send_stream.write_all(&message_data).await?;
        send_stream
            .finish()
            .map_err(|_| ChatError::connection("Failed to close request stream"))?;

        // Read response
        let response_data = recv_stream
            .read_to_end(self.config.max_message_size)
            .await
            .map_err(|e| ChatError::network(format!("Failed to read response: {}", e)))?;
        let response_message = serde_json::from_slice::<ChatMessage>(&response_data)
            .map_err(|e| ChatError::serialization(format!("Failed to parse response: {}", e)))?;

        // Send response as event
        if let Some(event_tx) = &self.event_tx {
            let _ = event_tx.send(ClientEvent::MessageReceived(response_message));
        }

        debug!("Requested user list");
        Ok(())
    }

    /// Disconnect from the chat server
    pub async fn disconnect(&mut self) -> Result<()> {
        if let Some(connection) = self.connection.take() {
            connection.close(0u32.into(), b"Client disconnect");
            info!("Disconnected from chat server");
        }

        if let Some(endpoint) = self.endpoint.take() {
            endpoint.close(0u32.into(), b"Client shutdown");
        }

        self.user = None;
        self.event_tx = None;

        Ok(())
    }

    /// Get the current user
    pub fn user(&self) -> Option<&User> {
        self.user.as_ref()
    }

    /// Check if connected to server
    pub fn is_connected(&self) -> bool {
        self.connection.is_some()
    }

    /// Get connection statistics
    pub fn connection_stats(&self) -> Option<ConnectionStats> {
        self.connection.as_ref().map(|conn| {
            let stats = conn.stats();
            ConnectionStats {
                bytes_sent: stats.udp_tx.bytes,
                bytes_received: stats.udp_rx.bytes,
                packets_sent: stats.udp_tx.datagrams,
                packets_received: stats.udp_rx.datagrams,
                round_trip_time: stats.path.rtt,
            }
        })
    }
}

/// Connection statistics
#[derive(Debug, Clone)]
pub struct ConnectionStats {
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub packets_sent: u64,
    pub packets_received: u64,
    pub round_trip_time: std::time::Duration,
}

/// Custom certificate verifier that accepts any certificate (INSECURE - for development only)
#[derive(Debug)]
struct AcceptAnyCertificate;

impl rustls::client::danger::ServerCertVerifier for AcceptAnyCertificate {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> std::result::Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        vec![
            rustls::SignatureScheme::RSA_PKCS1_SHA1,
            rustls::SignatureScheme::ECDSA_SHA1_Legacy,
            rustls::SignatureScheme::RSA_PKCS1_SHA256,
            rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
            rustls::SignatureScheme::RSA_PKCS1_SHA384,
            rustls::SignatureScheme::ECDSA_NISTP384_SHA384,
            rustls::SignatureScheme::RSA_PKCS1_SHA512,
            rustls::SignatureScheme::ECDSA_NISTP521_SHA512,
            rustls::SignatureScheme::RSA_PSS_SHA256,
            rustls::SignatureScheme::RSA_PSS_SHA384,
            rustls::SignatureScheme::RSA_PSS_SHA512,
            rustls::SignatureScheme::ED25519,
            rustls::SignatureScheme::ED448,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_config_default() {
        let config = ChatClientConfig::default();
        assert_eq!(config.server_addr.port(), 4433);
        assert_eq!(config.bind_addr.port(), 0);
        assert_eq!(config.connect_timeout_secs, 10);
        assert_eq!(config.max_message_size, 1024 * 1024);
    }

    #[test]
    fn test_client_creation() {
        let config = ChatClientConfig::default();
        let client = ChatClient::new(config.clone());

        assert_eq!(client.config.server_addr, config.server_addr);
        assert!(client.user.is_none());
        assert!(client.connection.is_none());
        assert!(!client.is_connected());
    }

    #[tokio::test]
    async fn test_client_disconnect() {
        let config = ChatClientConfig::default();
        let mut client = ChatClient::new(config);

        // Test disconnect when not connected
        assert!(client.disconnect().await.is_ok());
        assert!(!client.is_connected());
    }
}
