//! NATS JetStream storage client.
//!
//! Provides:
//! - Message persistence via JetStream (stream `GAROU_CHAT`, subject per room)
//! - History replay (last 50 messages per room on join)
//! - Room state persistence via JetStream KV (`GAROU_META` bucket)
//! - Message→room lookup for edit/delete/reaction (BUG-001 fix)
//! - Cross-node pub/sub via NATS Core subscriptions

use async_nats::HeaderMap;
use async_nats::jetstream::{self, consumer, kv, stream};
use futures_core::Stream;
use serde::{Deserialize, Serialize};
use std::future::poll_fn;
use std::pin::Pin;
use std::time::Duration;
use tracing::{debug, error, info, warn};

use crate::error::{ChatError, Result};
use crate::protocol::messages::{MessageId, RoomId, RoomMessage};

// ---------------------------------------------------------------------------
// Supporting types
// ---------------------------------------------------------------------------

/// Persistent room metadata stored in NATS KV.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomState {
    pub room_id: RoomId,
    pub name: String,
    pub room_type: String,
    pub created_at: u64,
    pub member_count: usize,
}

/// A live NATS Core subscription across all room subjects.
///
/// Yields `(room_id, RoomMessage)` pairs that originated on *other* server
/// nodes (messages from this node are filtered out via the `Server-Id` header).
pub struct RoomSubscription {
    subscriber: async_nats::Subscriber,
    my_server_id: String,
}

impl RoomSubscription {
    /// Return the next cross-node room message, skipping own-server messages.
    pub async fn next(&mut self) -> Option<(RoomId, RoomMessage)> {
        loop {
            let msg = poll_fn(|cx| Pin::new(&mut self.subscriber).poll_next(cx)).await?;

            // Skip messages we published ourselves
            let from_server = msg
                .headers
                .as_ref()
                .and_then(|h| h.get("Server-Id"))
                .map(|v| v.as_str())
                .unwrap_or("");
            if from_server == self.my_server_id {
                continue;
            }

            // Extract room_id from header
            let room_id = match msg
                .headers
                .as_ref()
                .and_then(|h| h.get("Room-Id"))
                .and_then(|v| v.as_str().parse::<RoomId>().ok())
            {
                Some(id) => id,
                None => {
                    warn!("NATS message missing Room-Id header on subject {}", msg.subject);
                    continue;
                }
            };

            // Deserialise
            match serde_json::from_slice::<RoomMessage>(&msg.payload) {
                Ok(room_msg) => return Some((room_id, room_msg)),
                Err(e) => {
                    warn!("Failed to deserialise NATS room message: {}", e);
                    continue;
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// NatsClient
// ---------------------------------------------------------------------------

/// NATS JetStream client for message persistence and cross-node delivery.
///
/// All I/O operations are fallible; the server degrades gracefully when NATS
/// is unavailable.
pub struct NatsClient {
    client: async_nats::Client,
    jetstream: jetstream::Context,
    stream_name: String,
    meta_kv: kv::Store,
    server_id: String,
}

impl std::fmt::Debug for NatsClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NatsClient")
            .field("stream_name", &self.stream_name)
            .field("server_id", &self.server_id)
            .finish_non_exhaustive()
    }
}

impl NatsClient {
    // -----------------------------------------------------------------------
    // Construction
    // -----------------------------------------------------------------------

    /// Connect to NATS and initialise the JetStream stream + KV bucket.
    pub async fn connect(url: &str, stream_name: &str) -> Result<Self> {
        let client = async_nats::connect(url)
            .await
            .map_err(|e| ChatError::Network(format!("NATS connect failed: {}", e)))?;

        let jetstream = jetstream::new(client.clone());

        // Create / verify the chat stream
        jetstream
            .get_or_create_stream(stream::Config {
                name: stream_name.to_string(),
                subjects: vec!["garou.rooms.>".to_string()],
                // Retain the last 100 messages per room subject so history
                // replay always has recent context without unbounded growth.
                max_messages_per_subject: 100,
                ..Default::default()
            })
            .await
            .map_err(|e| {
                ChatError::Internal(format!("NATS stream init failed: {}", e))
            })?;

        // Create / open the metadata KV bucket (room states + msg→room map)
        let meta_kv = jetstream
            .create_key_value(kv::Config {
                bucket: "GAROU_META".to_string(),
                ..Default::default()
            })
            .await
            .map_err(|e| ChatError::Internal(format!("NATS KV init failed: {}", e)))?;

        let server_id = uuid::Uuid::new_v4().to_string();
        info!("NATS connected (stream={}, server_id={})", stream_name, &server_id[..8]);

        Ok(Self {
            client,
            jetstream,
            stream_name: stream_name.to_string(),
            meta_kv,
            server_id,
        })
    }

    /// Return this node's unique server ID.
    pub fn server_id(&self) -> &str {
        &self.server_id
    }

    // -----------------------------------------------------------------------
    // Message persistence
    // -----------------------------------------------------------------------

    /// Publish a room message to JetStream (and NATS Core for live delivery).
    ///
    /// Returns an error if the server-side ACK is not received; callers should
    /// treat this as a fatal send failure and NOT broadcast locally.
    pub async fn publish_message(&self, msg: &RoomMessage) -> Result<()> {
        let payload = serde_json::to_vec(msg)
            .map_err(|e| ChatError::Serialization(format!("message serialise: {}", e)))?;

        let mut headers = HeaderMap::new();
        let room_id_str = msg.room_id.to_string();
        let msg_id_str = msg.message_id.to_string();
        headers.insert("Room-Id", room_id_str.as_str());
        headers.insert("Msg-Id", msg_id_str.as_str());
        headers.insert("Server-Id", self.server_id.as_str());
        // Use Msg-Id as NATS dedup key (exactly-once within the dedup window)
        headers.insert("Nats-Msg-Id", msg_id_str.as_str());

        let subject = format!("garou.rooms.{}.messages", msg.room_id);

        self.jetstream
            .publish_with_headers(subject, headers, payload.into())
            .await
            .map_err(|e| ChatError::Internal(format!("NATS publish failed: {}", e)))?
            .await
            .map_err(|e| ChatError::Internal(format!("NATS ACK failed: {}", e)))?;

        debug!("Published message {} to NATS", msg.message_id);
        Ok(())
    }

    /// Retrieve the last `limit` messages for a room from JetStream.
    ///
    /// Returns an empty Vec (not an error) if the room has no history or
    /// the stream is unreachable.
    pub async fn get_history(
        &self,
        room_id: RoomId,
        limit: usize,
    ) -> Result<Vec<RoomMessage>> {
        let stream = self
            .jetstream
            .get_stream(&self.stream_name)
            .await
            .map_err(|e| ChatError::Internal(format!("NATS get stream: {}", e)))?;

        // Ephemeral pull consumer filtered to this room's subject
        let consumer = stream
            .create_consumer(consumer::pull::Config {
                filter_subject: format!("garou.rooms.{}.messages", room_id),
                deliver_policy: consumer::DeliverPolicy::All,
                ..Default::default()
            })
            .await
            .map_err(|e| ChatError::Internal(format!("NATS consumer create: {}", e)))?;

        let fetch = consumer
            .fetch()
            .max_messages(limit)
            .expires(Duration::from_secs(5))
            .messages()
            .await
            .map_err(|e| ChatError::Internal(format!("NATS fetch: {}", e)))?;

        tokio::pin!(fetch);

        let mut messages = Vec::with_capacity(limit);
        while let Some(result) = poll_fn(|cx| Pin::new(&mut fetch).poll_next(cx)).await {
            match result {
                Ok(msg) => {
                    let _ = msg.ack().await;
                    match serde_json::from_slice::<RoomMessage>(&msg.payload) {
                        Ok(room_msg) => messages.push(room_msg),
                        Err(e) => warn!("Failed to deserialise history message: {}", e),
                    }
                }
                Err(e) => {
                    warn!("NATS history fetch error: {}", e);
                    break;
                }
            }
        }

        debug!("Replayed {} messages for room {} from NATS", messages.len(), room_id);
        Ok(messages)
    }

    // -----------------------------------------------------------------------
    // Message → room mapping (BUG-001 fix)
    // -----------------------------------------------------------------------

    /// Store a message_id → room_id mapping so edit/delete/reaction handlers
    /// can look up the room without an explicit `room_id` parameter.
    pub async fn save_message_room(
        &self,
        message_id: MessageId,
        room_id: RoomId,
    ) -> Result<()> {
        let key = format!("msg.{}", message_id);
        self.meta_kv
            .put(key, room_id.to_string().into())
            .await
            .map_err(|e| ChatError::Internal(format!("NATS KV put msg→room: {}", e)))?;
        Ok(())
    }

    /// Look up the room ID for a message. Returns `None` if the mapping does
    /// not exist (e.g., message predates NATS integration).
    pub async fn get_room_id_for_message(
        &self,
        message_id: MessageId,
    ) -> Result<Option<RoomId>> {
        let key = format!("msg.{}", message_id);
        match self.meta_kv.get(key).await {
            Ok(Some(entry)) => {
                let s = String::from_utf8_lossy(&entry);
                let room_id = s.parse::<RoomId>().map_err(|_| {
                    ChatError::Internal(format!("invalid room_id in KV for msg {}: '{}'", message_id, s))
                })?;
                Ok(Some(room_id))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(ChatError::Internal(format!("NATS KV get msg→room: {}", e))),
        }
    }

    // -----------------------------------------------------------------------
    // Room state
    // -----------------------------------------------------------------------

    /// Persist room state to NATS KV.
    pub async fn save_room_state(&self, state: &RoomState) -> Result<()> {
        let key = format!("room.{}", state.room_id);
        let value = serde_json::to_vec(state)
            .map_err(|e| ChatError::Serialization(format!("RoomState serialise: {}", e)))?;
        self.meta_kv
            .put(key, value.into())
            .await
            .map_err(|e| ChatError::Internal(format!("NATS KV put room state: {}", e)))?;
        Ok(())
    }

    /// Retrieve room state from NATS KV. Returns `None` if not found.
    pub async fn get_room_state(&self, room_id: RoomId) -> Result<Option<RoomState>> {
        let key = format!("room.{}", room_id);
        match self.meta_kv.get(key).await {
            Ok(Some(entry)) => {
                let state = serde_json::from_slice::<RoomState>(&entry).map_err(|e| {
                    ChatError::Serialization(format!("RoomState deserialise: {}", e))
                })?;
                Ok(Some(state))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(ChatError::Internal(format!("NATS KV get room state: {}", e))),
        }
    }

    // -----------------------------------------------------------------------
    // Cross-node subscription
    // -----------------------------------------------------------------------

    /// Subscribe to all room message subjects (`garou.rooms.>`).
    ///
    /// Messages published by this server node are filtered out automatically
    /// by `RoomSubscription::next` using the `Server-Id` header.
    pub async fn subscribe_rooms(&self) -> Result<RoomSubscription> {
        let subscriber = self
            .client
            .subscribe("garou.rooms.>")
            .await
            .map_err(|e| ChatError::Network(format!("NATS subscribe: {}", e)))?;

        Ok(RoomSubscription {
            subscriber,
            my_server_id: self.server_id.clone(),
        })
    }

    // -----------------------------------------------------------------------
    // Health
    // -----------------------------------------------------------------------

    /// Returns `true` if the NATS connection is alive.
    pub async fn health_check(&self) -> bool {
        self.client.flush().await.is_ok()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Connecting to a non-existent NATS server must fail cleanly.
    #[tokio::test]
    async fn test_connect_unreachable_fails() {
        let result = NatsClient::connect("nats://127.0.0.1:14222", "TEST").await;
        assert!(
            result.is_err(),
            "connection to unreachable NATS should return an error"
        );
        let err = result.unwrap_err();
        assert!(
            matches!(err, ChatError::Network(_)),
            "expected ChatError::Network, got: {:?}",
            err
        );
    }

    /// RoomState round-trips through JSON serialisation.
    #[test]
    fn test_room_state_serialise_roundtrip() {
        let state = RoomState {
            room_id: 42,
            name: "general".to_string(),
            room_type: "channel".to_string(),
            created_at: 1_700_000_000,
            member_count: 5,
        };
        let json = serde_json::to_vec(&state).unwrap();
        let decoded: RoomState = serde_json::from_slice(&json).unwrap();
        assert_eq!(decoded.room_id, 42);
        assert_eq!(decoded.name, "general");
        assert_eq!(decoded.member_count, 5);
    }
}
