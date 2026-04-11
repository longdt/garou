//! Codec for encoding/decoding protocol messages to/from binary frames.
//!
//! **Default** (no features): FlatBuffers wire format.
//! **`debug-json` feature**: JSON wire format (easier to inspect with tools).
//!
//! The [`Encodable`] / [`Decodable`] traits are the same in both modes; only
//! the serialization backend changes.

use super::frame::{Frame, FrameType};
use super::messages::*;
use bytes::Bytes;
use std::io::{self, Error as IoError, ErrorKind};

// ---------------------------------------------------------------------------
// Traits (mode-independent)
// ---------------------------------------------------------------------------

/// Messages that can be serialized into a [`Frame`].
pub trait Encodable {
    fn frame_type(&self) -> FrameType;
    fn encode_payload(&self) -> io::Result<Bytes>;
    fn encode_frame(&self) -> io::Result<Frame> {
        Ok(Frame::new(self.frame_type(), self.encode_payload()?))
    }
}

/// Messages that can be deserialized from a [`Frame`] payload.
pub trait Decodable: Sized {
    fn expected_frame_type() -> FrameType;
    fn decode_payload(payload: &[u8]) -> io::Result<Self>;
    fn decode_frame(frame: &Frame) -> io::Result<Self> {
        if frame.frame_type != Self::expected_frame_type() {
            return Err(IoError::new(
                ErrorKind::InvalidData,
                format!(
                    "Expected frame type {:?}, got {:?}",
                    Self::expected_frame_type(),
                    frame.frame_type
                ),
            ));
        }
        Self::decode_payload(&frame.payload)
    }
}

// ---------------------------------------------------------------------------
// JSON codec  (only compiled with `--features debug-json`)
// ---------------------------------------------------------------------------

#[cfg(feature = "debug-json")]
macro_rules! impl_codec {
    ($type:ty, $frame_type:expr) => {
        impl Encodable for $type {
            fn frame_type(&self) -> FrameType { $frame_type }
            fn encode_payload(&self) -> io::Result<Bytes> {
                serde_json::to_vec(self)
                    .map(Bytes::from)
                    .map_err(|e| IoError::new(ErrorKind::InvalidData, e))
            }
        }
        impl Decodable for $type {
            fn expected_frame_type() -> FrameType { $frame_type }
            fn decode_payload(payload: &[u8]) -> io::Result<Self> {
                serde_json::from_slice(payload).map_err(|e| IoError::new(ErrorKind::InvalidData, e))
            }
        }
    };
}

#[cfg(feature = "debug-json")]
impl_codec!(Hello,              FrameType::Hello);
#[cfg(feature = "debug-json")]
impl_codec!(HelloAck,           FrameType::HelloAck);
#[cfg(feature = "debug-json")]
impl_codec!(Auth,               FrameType::Auth);
#[cfg(feature = "debug-json")]
impl_codec!(AuthOk,             FrameType::AuthOk);
#[cfg(feature = "debug-json")]
impl_codec!(AuthFailed,         FrameType::AuthFailed);
#[cfg(feature = "debug-json")]
impl_codec!(Ping,               FrameType::Ping);
#[cfg(feature = "debug-json")]
impl_codec!(Pong,               FrameType::Pong);
#[cfg(feature = "debug-json")]
impl_codec!(Throttle,           FrameType::Throttle);
#[cfg(feature = "debug-json")]
impl_codec!(Goodbye,            FrameType::Goodbye);
#[cfg(feature = "debug-json")]
impl_codec!(ServerCommand,      FrameType::ServerCommand);
#[cfg(feature = "debug-json")]
impl_codec!(SendMessage,        FrameType::SendMessage);
#[cfg(feature = "debug-json")]
impl_codec!(EditMessage,        FrameType::EditMessage);
#[cfg(feature = "debug-json")]
impl_codec!(DeleteMessage,      FrameType::DeleteMessage);
#[cfg(feature = "debug-json")]
impl_codec!(AddReaction,        FrameType::AddReaction);
#[cfg(feature = "debug-json")]
impl_codec!(RemoveReaction,     FrameType::RemoveReaction);
#[cfg(feature = "debug-json")]
impl_codec!(JoinRoom,           FrameType::JoinRoom);
#[cfg(feature = "debug-json")]
impl_codec!(LeaveRoom,          FrameType::LeaveRoom);
#[cfg(feature = "debug-json")]
impl_codec!(CreateRoom,         FrameType::CreateRoom);
#[cfg(feature = "debug-json")]
impl_codec!(RoomMessage,        FrameType::RoomMessage);
#[cfg(feature = "debug-json")]
impl_codec!(RoomMessageEdited,  FrameType::RoomMessageEdited);
#[cfg(feature = "debug-json")]
impl_codec!(RoomMessageDeleted, FrameType::RoomMessageDeleted);
#[cfg(feature = "debug-json")]
impl_codec!(RoomReactionAdded,  FrameType::RoomReactionAdded);
#[cfg(feature = "debug-json")]
impl_codec!(RoomReactionRemoved,FrameType::RoomReactionRemoved);
#[cfg(feature = "debug-json")]
impl_codec!(RoomUserJoined,     FrameType::RoomUserJoined);
#[cfg(feature = "debug-json")]
impl_codec!(RoomUserLeft,       FrameType::RoomUserLeft);
#[cfg(feature = "debug-json")]
impl_codec!(RoomInit,           FrameType::RoomInit);
#[cfg(feature = "debug-json")]
impl_codec!(RoomClose,          FrameType::RoomClose);
#[cfg(feature = "debug-json")]
impl_codec!(ShardAssignment,    FrameType::ShardAssignment);
#[cfg(feature = "debug-json")]
impl_codec!(RoomPromoted,       FrameType::RoomPromoted);
#[cfg(feature = "debug-json")]
impl_codec!(RoomDemoted,        FrameType::RoomDemoted);
#[cfg(feature = "debug-json")]
impl_codec!(MessageDelivered,   FrameType::MessageDelivered);
#[cfg(feature = "debug-json")]
impl_codec!(MessageRead,        FrameType::MessageRead);
#[cfg(feature = "debug-json")]
impl_codec!(MessageAck,         FrameType::MessageAck);
#[cfg(feature = "debug-json")]
impl_codec!(UploadStart,        FrameType::UploadStart);
#[cfg(feature = "debug-json")]
impl_codec!(UploadChunk,        FrameType::UploadChunk);
#[cfg(feature = "debug-json")]
impl_codec!(UploadComplete,     FrameType::UploadComplete);
#[cfg(feature = "debug-json")]
impl_codec!(UploadCancel,       FrameType::UploadCancel);
#[cfg(feature = "debug-json")]
impl_codec!(UploadAck,          FrameType::UploadAck);
#[cfg(feature = "debug-json")]
impl_codec!(Typing,             FrameType::Typing);
#[cfg(feature = "debug-json")]
impl_codec!(StopTyping,         FrameType::StopTyping);
#[cfg(feature = "debug-json")]
impl_codec!(PresenceOnline,     FrameType::PresenceOnline);
#[cfg(feature = "debug-json")]
impl_codec!(PresenceOffline,    FrameType::PresenceOffline);
#[cfg(feature = "debug-json")]
impl_codec!(PresenceAway,       FrameType::PresenceAway);
#[cfg(feature = "debug-json")]
impl_codec!(Error,              FrameType::Error);

// ---------------------------------------------------------------------------
// FlatBuffers codec  (default, no feature flag)
// ---------------------------------------------------------------------------

#[cfg(not(feature = "debug-json"))]
mod fb {
    use super::super::generated::{
        ack_generated::garou as fg_ack,
        chat_generated::garou as fg_chat,
        common_generated::garou as fg_common,
        control_generated::garou as fg_ctrl,
        error_generated::garou as fg_err,
        presence_generated::garou as fg_presence,
        room_generated::garou as fg_room,
        shard_generated::garou as fg_shard,
        upload_generated::garou as fg_upload,
    };
    use super::super::messages::*;
    use bytes::Bytes;
    use flatbuffers::FlatBufferBuilder;
    use std::io::{self, Error as IoError, ErrorKind};

    // -----------------------------------------------------------------------
    // Helper: convert IoError for flatbuffers InvalidFlatbuffer
    // -----------------------------------------------------------------------
    fn fb_err(e: flatbuffers::InvalidFlatbuffer) -> IoError {
        IoError::new(ErrorKind::InvalidData, format!("flatbuffers: {:?}", e))
    }

    fn empty_str_err() -> IoError {
        IoError::new(ErrorKind::InvalidData, "flatbuffers: required string field absent")
    }

    // -----------------------------------------------------------------------
    // Helper: build/decode UserInfo nested type
    // -----------------------------------------------------------------------
    fn build_user_info<'bldr>(
        b: &mut FlatBufferBuilder<'bldr>,
        u: &UserInfo,
    ) -> flatbuffers::WIPOffset<fg_common::UserInfo<'bldr>> {
        let username = b.create_string(&u.username);
        let avatar = u.avatar_url.as_deref().map(|s| b.create_string(s));
        fg_common::UserInfo::create(
            b,
            &fg_common::UserInfoArgs {
                user_id: u.user_id,
                username: Some(username),
                avatar_url: avatar,
            },
        )
    }

    fn decode_user_info(fb: fg_common::UserInfo<'_>) -> io::Result<UserInfo> {
        Ok(UserInfo {
            user_id: fb.user_id(),
            username: fb.username().ok_or_else(empty_str_err)?.to_string(),
            avatar_url: fb.avatar_url().map(|s| s.to_string()),
        })
    }

    // -----------------------------------------------------------------------
    // Control messages
    // -----------------------------------------------------------------------

    pub fn enc_hello(msg: &Hello) -> io::Result<Bytes> {
        let mut b = FlatBufferBuilder::with_capacity(128);
        let caps: Vec<_> = msg.capabilities.iter().map(|s| b.create_string(s)).collect();
        let caps_vec = b.create_vector(&caps);
        let root = fg_ctrl::Hello::create(
            &mut b,
            &fg_ctrl::HelloArgs { version: msg.version, capabilities: Some(caps_vec) },
        );
        b.finish(root, None);
        Ok(Bytes::copy_from_slice(b.finished_data()))
    }
    pub fn dec_hello(p: &[u8]) -> io::Result<Hello> {
        let fb = flatbuffers::root::<fg_ctrl::Hello>(p).map_err(fb_err)?;
        Ok(Hello {
            version: fb.version(),
            capabilities: fb
                .capabilities()
                .map(|v| v.iter().map(|s| s.to_string()).collect())
                .unwrap_or_default(),
        })
    }

    pub fn enc_hello_ack(msg: &HelloAck) -> io::Result<Bytes> {
        let mut b = FlatBufferBuilder::with_capacity(128);
        let session_id = b.create_string(&msg.session_id);
        let root = fg_ctrl::HelloAck::create(
            &mut b,
            &fg_ctrl::HelloAckArgs {
                version: msg.version,
                session_id: Some(session_id),
                num_shards: msg.num_shards,
            },
        );
        b.finish(root, None);
        Ok(Bytes::copy_from_slice(b.finished_data()))
    }
    pub fn dec_hello_ack(p: &[u8]) -> io::Result<HelloAck> {
        let fb = flatbuffers::root::<fg_ctrl::HelloAck>(p).map_err(fb_err)?;
        Ok(HelloAck {
            version: fb.version(),
            session_id: fb.session_id().ok_or_else(empty_str_err)?.to_string(),
            num_shards: fb.num_shards(),
        })
    }

    pub fn enc_auth(msg: &Auth) -> io::Result<Bytes> {
        let mut b = FlatBufferBuilder::with_capacity(128);
        let method = b.create_string(&msg.method);
        let creds = b.create_string(&msg.credentials);
        let root = fg_ctrl::Auth::create(
            &mut b,
            &fg_ctrl::AuthArgs { method: Some(method), credentials: Some(creds) },
        );
        b.finish(root, None);
        Ok(Bytes::copy_from_slice(b.finished_data()))
    }
    pub fn dec_auth(p: &[u8]) -> io::Result<Auth> {
        let fb = flatbuffers::root::<fg_ctrl::Auth>(p).map_err(fb_err)?;
        Ok(Auth {
            method: fb.method().ok_or_else(empty_str_err)?.to_string(),
            credentials: fb.credentials().ok_or_else(empty_str_err)?.to_string(),
        })
    }

    pub fn enc_auth_ok(msg: &AuthOk) -> io::Result<Bytes> {
        let mut b = FlatBufferBuilder::with_capacity(128);
        let username = b.create_string(&msg.username);
        let rooms = b.create_vector(&msg.rooms);
        let root = fg_ctrl::AuthOk::create(
            &mut b,
            &fg_ctrl::AuthOkArgs {
                user_id: msg.user_id,
                username: Some(username),
                rooms: Some(rooms),
            },
        );
        b.finish(root, None);
        Ok(Bytes::copy_from_slice(b.finished_data()))
    }
    pub fn dec_auth_ok(p: &[u8]) -> io::Result<AuthOk> {
        let fb = flatbuffers::root::<fg_ctrl::AuthOk>(p).map_err(fb_err)?;
        Ok(AuthOk {
            user_id: fb.user_id(),
            username: fb.username().ok_or_else(empty_str_err)?.to_string(),
            rooms: fb.rooms().map(|v| v.iter().collect()).unwrap_or_default(),
        })
    }

    pub fn enc_auth_failed(msg: &AuthFailed) -> io::Result<Bytes> {
        let mut b = FlatBufferBuilder::with_capacity(64);
        let message = b.create_string(&msg.message);
        let root = fg_ctrl::AuthFailed::create(
            &mut b,
            &fg_ctrl::AuthFailedArgs { code: msg.code, message: Some(message) },
        );
        b.finish(root, None);
        Ok(Bytes::copy_from_slice(b.finished_data()))
    }
    pub fn dec_auth_failed(p: &[u8]) -> io::Result<AuthFailed> {
        let fb = flatbuffers::root::<fg_ctrl::AuthFailed>(p).map_err(fb_err)?;
        Ok(AuthFailed {
            code: fb.code(),
            message: fb.message().ok_or_else(empty_str_err)?.to_string(),
        })
    }

    pub fn enc_ping(msg: &Ping) -> io::Result<Bytes> {
        let mut b = FlatBufferBuilder::with_capacity(32);
        let root = fg_ctrl::Ping::create(&mut b, &fg_ctrl::PingArgs { timestamp: msg.timestamp });
        b.finish(root, None);
        Ok(Bytes::copy_from_slice(b.finished_data()))
    }
    pub fn dec_ping(p: &[u8]) -> io::Result<Ping> {
        let fb = flatbuffers::root::<fg_ctrl::Ping>(p).map_err(fb_err)?;
        Ok(Ping { timestamp: fb.timestamp() })
    }

    pub fn enc_pong(msg: &Pong) -> io::Result<Bytes> {
        let mut b = FlatBufferBuilder::with_capacity(32);
        let root = fg_ctrl::Pong::create(&mut b, &fg_ctrl::PongArgs { timestamp: msg.timestamp });
        b.finish(root, None);
        Ok(Bytes::copy_from_slice(b.finished_data()))
    }
    pub fn dec_pong(p: &[u8]) -> io::Result<Pong> {
        let fb = flatbuffers::root::<fg_ctrl::Pong>(p).map_err(fb_err)?;
        Ok(Pong { timestamp: fb.timestamp() })
    }

    pub fn enc_throttle(msg: &Throttle) -> io::Result<Bytes> {
        let mut b = FlatBufferBuilder::with_capacity(64);
        let reason = b.create_string(&msg.reason);
        let root = fg_ctrl::Throttle::create(
            &mut b,
            &fg_ctrl::ThrottleArgs {
                has_room_id: msg.room_id.is_some(),
                room_id: msg.room_id.unwrap_or(0),
                delay_ms: msg.delay_ms,
                reason: Some(reason),
            },
        );
        b.finish(root, None);
        Ok(Bytes::copy_from_slice(b.finished_data()))
    }
    pub fn dec_throttle(p: &[u8]) -> io::Result<Throttle> {
        let fb = flatbuffers::root::<fg_ctrl::Throttle>(p).map_err(fb_err)?;
        Ok(Throttle {
            room_id: if fb.has_room_id() { Some(fb.room_id()) } else { None },
            delay_ms: fb.delay_ms(),
            reason: fb.reason().ok_or_else(empty_str_err)?.to_string(),
        })
    }

    pub fn enc_goodbye(msg: &Goodbye) -> io::Result<Bytes> {
        let mut b = FlatBufferBuilder::with_capacity(64);
        let reason = b.create_string(&msg.reason);
        let root =
            fg_ctrl::Goodbye::create(&mut b, &fg_ctrl::GoodbyeArgs { reason: Some(reason) });
        b.finish(root, None);
        Ok(Bytes::copy_from_slice(b.finished_data()))
    }
    pub fn dec_goodbye(p: &[u8]) -> io::Result<Goodbye> {
        let fb = flatbuffers::root::<fg_ctrl::Goodbye>(p).map_err(fb_err)?;
        Ok(Goodbye { reason: fb.reason().ok_or_else(empty_str_err)?.to_string() })
    }

    pub fn enc_server_command(msg: &ServerCommand) -> io::Result<Bytes> {
        let params_json = serde_json::to_string(&msg.params)
            .map_err(|e| IoError::new(ErrorKind::InvalidData, e))?;
        let mut b = FlatBufferBuilder::with_capacity(128);
        let command = b.create_string(&msg.command);
        let params = b.create_string(&params_json);
        let root = fg_ctrl::ServerCommand::create(
            &mut b,
            &fg_ctrl::ServerCommandArgs { command: Some(command), params: Some(params) },
        );
        b.finish(root, None);
        Ok(Bytes::copy_from_slice(b.finished_data()))
    }
    pub fn dec_server_command(p: &[u8]) -> io::Result<ServerCommand> {
        let fb = flatbuffers::root::<fg_ctrl::ServerCommand>(p).map_err(fb_err)?;
        let params_str = fb.params().unwrap_or("null");
        let params: serde_json::Value = serde_json::from_str(params_str)
            .map_err(|e| IoError::new(ErrorKind::InvalidData, e))?;
        Ok(ServerCommand {
            command: fb.command().ok_or_else(empty_str_err)?.to_string(),
            params,
        })
    }

    // -----------------------------------------------------------------------
    // Chat command messages
    // -----------------------------------------------------------------------

    pub fn enc_send_message(msg: &SendMessage) -> io::Result<Bytes> {
        let mut b = FlatBufferBuilder::with_capacity(256);
        let content = b.create_string(&msg.content);
        let nonce = b.create_string(&msg.nonce);
        let attachments = b.create_vector(&msg.attachments);
        let root = fg_chat::SendMessage::create(
            &mut b,
            &fg_chat::SendMessageArgs {
                room_id: msg.room_id,
                content: Some(content),
                nonce: Some(nonce),
                has_reply_to: msg.reply_to.is_some(),
                reply_to: msg.reply_to.unwrap_or(0),
                attachments: Some(attachments),
            },
        );
        b.finish(root, None);
        Ok(Bytes::copy_from_slice(b.finished_data()))
    }
    pub fn dec_send_message(p: &[u8]) -> io::Result<SendMessage> {
        let fb = flatbuffers::root::<fg_chat::SendMessage>(p).map_err(fb_err)?;
        Ok(SendMessage {
            room_id: fb.room_id(),
            content: fb.content().ok_or_else(empty_str_err)?.to_string(),
            nonce: fb.nonce().ok_or_else(empty_str_err)?.to_string(),
            reply_to: if fb.has_reply_to() { Some(fb.reply_to()) } else { None },
            attachments: fb.attachments().map(|v| v.iter().collect()).unwrap_or_default(),
        })
    }

    pub fn enc_edit_message(msg: &EditMessage) -> io::Result<Bytes> {
        let mut b = FlatBufferBuilder::with_capacity(128);
        let content = b.create_string(&msg.content);
        let root = fg_chat::EditMessage::create(
            &mut b,
            &fg_chat::EditMessageArgs { message_id: msg.message_id, content: Some(content) },
        );
        b.finish(root, None);
        Ok(Bytes::copy_from_slice(b.finished_data()))
    }
    pub fn dec_edit_message(p: &[u8]) -> io::Result<EditMessage> {
        let fb = flatbuffers::root::<fg_chat::EditMessage>(p).map_err(fb_err)?;
        Ok(EditMessage {
            message_id: fb.message_id(),
            content: fb.content().ok_or_else(empty_str_err)?.to_string(),
        })
    }

    pub fn enc_delete_message(msg: &DeleteMessage) -> io::Result<Bytes> {
        let mut b = FlatBufferBuilder::with_capacity(32);
        let root = fg_chat::DeleteMessage::create(
            &mut b,
            &fg_chat::DeleteMessageArgs { message_id: msg.message_id },
        );
        b.finish(root, None);
        Ok(Bytes::copy_from_slice(b.finished_data()))
    }
    pub fn dec_delete_message(p: &[u8]) -> io::Result<DeleteMessage> {
        let fb = flatbuffers::root::<fg_chat::DeleteMessage>(p).map_err(fb_err)?;
        Ok(DeleteMessage { message_id: fb.message_id() })
    }

    pub fn enc_add_reaction(msg: &AddReaction) -> io::Result<Bytes> {
        let mut b = FlatBufferBuilder::with_capacity(64);
        let emoji = b.create_string(&msg.emoji);
        let root = fg_chat::AddReaction::create(
            &mut b,
            &fg_chat::AddReactionArgs { message_id: msg.message_id, emoji: Some(emoji) },
        );
        b.finish(root, None);
        Ok(Bytes::copy_from_slice(b.finished_data()))
    }
    pub fn dec_add_reaction(p: &[u8]) -> io::Result<AddReaction> {
        let fb = flatbuffers::root::<fg_chat::AddReaction>(p).map_err(fb_err)?;
        Ok(AddReaction {
            message_id: fb.message_id(),
            emoji: fb.emoji().ok_or_else(empty_str_err)?.to_string(),
        })
    }

    pub fn enc_remove_reaction(msg: &RemoveReaction) -> io::Result<Bytes> {
        let mut b = FlatBufferBuilder::with_capacity(64);
        let emoji = b.create_string(&msg.emoji);
        let root = fg_chat::RemoveReaction::create(
            &mut b,
            &fg_chat::RemoveReactionArgs { message_id: msg.message_id, emoji: Some(emoji) },
        );
        b.finish(root, None);
        Ok(Bytes::copy_from_slice(b.finished_data()))
    }
    pub fn dec_remove_reaction(p: &[u8]) -> io::Result<RemoveReaction> {
        let fb = flatbuffers::root::<fg_chat::RemoveReaction>(p).map_err(fb_err)?;
        Ok(RemoveReaction {
            message_id: fb.message_id(),
            emoji: fb.emoji().ok_or_else(empty_str_err)?.to_string(),
        })
    }

    pub fn enc_join_room(msg: &JoinRoom) -> io::Result<Bytes> {
        let mut b = FlatBufferBuilder::with_capacity(32);
        let root =
            fg_chat::JoinRoom::create(&mut b, &fg_chat::JoinRoomArgs { room_id: msg.room_id });
        b.finish(root, None);
        Ok(Bytes::copy_from_slice(b.finished_data()))
    }
    pub fn dec_join_room(p: &[u8]) -> io::Result<JoinRoom> {
        let fb = flatbuffers::root::<fg_chat::JoinRoom>(p).map_err(fb_err)?;
        Ok(JoinRoom { room_id: fb.room_id() })
    }

    pub fn enc_leave_room(msg: &LeaveRoom) -> io::Result<Bytes> {
        let mut b = FlatBufferBuilder::with_capacity(32);
        let root =
            fg_chat::LeaveRoom::create(&mut b, &fg_chat::LeaveRoomArgs { room_id: msg.room_id });
        b.finish(root, None);
        Ok(Bytes::copy_from_slice(b.finished_data()))
    }
    pub fn dec_leave_room(p: &[u8]) -> io::Result<LeaveRoom> {
        let fb = flatbuffers::root::<fg_chat::LeaveRoom>(p).map_err(fb_err)?;
        Ok(LeaveRoom { room_id: fb.room_id() })
    }

    pub fn enc_create_room(msg: &CreateRoom) -> io::Result<Bytes> {
        let mut b = FlatBufferBuilder::with_capacity(128);
        let name = b.create_string(&msg.name);
        let room_type = b.create_string(&msg.room_type);
        let members = b.create_vector(&msg.members);
        let root = fg_chat::CreateRoom::create(
            &mut b,
            &fg_chat::CreateRoomArgs {
                name: Some(name),
                room_type: Some(room_type),
                members: Some(members),
            },
        );
        b.finish(root, None);
        Ok(Bytes::copy_from_slice(b.finished_data()))
    }
    pub fn dec_create_room(p: &[u8]) -> io::Result<CreateRoom> {
        let fb = flatbuffers::root::<fg_chat::CreateRoom>(p).map_err(fb_err)?;
        Ok(CreateRoom {
            name: fb.name().ok_or_else(empty_str_err)?.to_string(),
            room_type: fb.room_type().ok_or_else(empty_str_err)?.to_string(),
            members: fb.members().map(|v| v.iter().collect()).unwrap_or_default(),
        })
    }

    // -----------------------------------------------------------------------
    // Room messages
    // -----------------------------------------------------------------------

    fn build_room_message<'bldr>(
        b: &mut FlatBufferBuilder<'bldr>,
        msg: &RoomMessage,
    ) -> flatbuffers::WIPOffset<fg_room::RoomMessage<'bldr>> {
        let sender = build_user_info(b, &msg.sender);
        let content = b.create_string(&msg.content);
        let attachments: Vec<_> = msg.attachments.iter().map(|s| b.create_string(s)).collect();
        let attachments_vec = b.create_vector(&attachments);
        let nonce = msg.nonce.as_deref().map(|s| b.create_string(s));
        fg_room::RoomMessage::create(
            b,
            &fg_room::RoomMessageArgs {
                message_id: msg.message_id,
                room_id: msg.room_id,
                sender: Some(sender),
                content: Some(content),
                timestamp: msg.timestamp,
                has_reply_to: msg.reply_to.is_some(),
                reply_to: msg.reply_to.unwrap_or(0),
                attachments: Some(attachments_vec),
                has_nonce: msg.nonce.is_some(),
                nonce,
            },
        )
    }

    fn decode_room_message_fb(fb: fg_room::RoomMessage<'_>) -> io::Result<RoomMessage> {
        let sender = fb.sender().ok_or_else(|| {
            IoError::new(ErrorKind::InvalidData, "flatbuffers: sender absent")
        })?;
        Ok(RoomMessage {
            message_id: fb.message_id(),
            room_id: fb.room_id(),
            sender: decode_user_info(sender)?,
            content: fb.content().ok_or_else(empty_str_err)?.to_string(),
            timestamp: fb.timestamp(),
            reply_to: if fb.has_reply_to() { Some(fb.reply_to()) } else { None },
            attachments: fb
                .attachments()
                .map(|v| v.iter().map(|s| s.to_string()).collect())
                .unwrap_or_default(),
            nonce: if fb.has_nonce() { fb.nonce().map(|s| s.to_string()) } else { None },
        })
    }

    pub fn enc_room_message(msg: &RoomMessage) -> io::Result<Bytes> {
        let mut b = FlatBufferBuilder::with_capacity(512);
        let root = build_room_message(&mut b, msg);
        b.finish(root, None);
        Ok(Bytes::copy_from_slice(b.finished_data()))
    }
    pub fn dec_room_message(p: &[u8]) -> io::Result<RoomMessage> {
        let fb = flatbuffers::root::<fg_room::RoomMessage>(p).map_err(fb_err)?;
        decode_room_message_fb(fb)
    }

    pub fn enc_room_message_edited(msg: &RoomMessageEdited) -> io::Result<Bytes> {
        let mut b = FlatBufferBuilder::with_capacity(128);
        let content = b.create_string(&msg.content);
        let root = fg_room::RoomMessageEdited::create(
            &mut b,
            &fg_room::RoomMessageEditedArgs {
                message_id: msg.message_id,
                room_id: msg.room_id,
                editor_id: msg.editor_id,
                content: Some(content),
                edited_at: msg.edited_at,
            },
        );
        b.finish(root, None);
        Ok(Bytes::copy_from_slice(b.finished_data()))
    }
    pub fn dec_room_message_edited(p: &[u8]) -> io::Result<RoomMessageEdited> {
        let fb = flatbuffers::root::<fg_room::RoomMessageEdited>(p).map_err(fb_err)?;
        Ok(RoomMessageEdited {
            message_id: fb.message_id(),
            room_id: fb.room_id(),
            editor_id: fb.editor_id(),
            content: fb.content().ok_or_else(empty_str_err)?.to_string(),
            edited_at: fb.edited_at(),
        })
    }

    pub fn enc_room_message_deleted(msg: &RoomMessageDeleted) -> io::Result<Bytes> {
        let mut b = FlatBufferBuilder::with_capacity(64);
        let root = fg_room::RoomMessageDeleted::create(
            &mut b,
            &fg_room::RoomMessageDeletedArgs {
                message_id: msg.message_id,
                room_id: msg.room_id,
                deleted_by: msg.deleted_by,
                deleted_at: msg.deleted_at,
            },
        );
        b.finish(root, None);
        Ok(Bytes::copy_from_slice(b.finished_data()))
    }
    pub fn dec_room_message_deleted(p: &[u8]) -> io::Result<RoomMessageDeleted> {
        let fb = flatbuffers::root::<fg_room::RoomMessageDeleted>(p).map_err(fb_err)?;
        Ok(RoomMessageDeleted {
            message_id: fb.message_id(),
            room_id: fb.room_id(),
            deleted_by: fb.deleted_by(),
            deleted_at: fb.deleted_at(),
        })
    }

    pub fn enc_room_reaction_added(msg: &RoomReactionAdded) -> io::Result<Bytes> {
        let mut b = FlatBufferBuilder::with_capacity(64);
        let emoji = b.create_string(&msg.emoji);
        let root = fg_room::RoomReactionAdded::create(
            &mut b,
            &fg_room::RoomReactionAddedArgs {
                message_id: msg.message_id,
                room_id: msg.room_id,
                user_id: msg.user_id,
                emoji: Some(emoji),
            },
        );
        b.finish(root, None);
        Ok(Bytes::copy_from_slice(b.finished_data()))
    }
    pub fn dec_room_reaction_added(p: &[u8]) -> io::Result<RoomReactionAdded> {
        let fb = flatbuffers::root::<fg_room::RoomReactionAdded>(p).map_err(fb_err)?;
        Ok(RoomReactionAdded {
            message_id: fb.message_id(),
            room_id: fb.room_id(),
            user_id: fb.user_id(),
            emoji: fb.emoji().ok_or_else(empty_str_err)?.to_string(),
        })
    }

    pub fn enc_room_reaction_removed(msg: &RoomReactionRemoved) -> io::Result<Bytes> {
        let mut b = FlatBufferBuilder::with_capacity(64);
        let emoji = b.create_string(&msg.emoji);
        let root = fg_room::RoomReactionRemoved::create(
            &mut b,
            &fg_room::RoomReactionRemovedArgs {
                message_id: msg.message_id,
                room_id: msg.room_id,
                user_id: msg.user_id,
                emoji: Some(emoji),
            },
        );
        b.finish(root, None);
        Ok(Bytes::copy_from_slice(b.finished_data()))
    }
    pub fn dec_room_reaction_removed(p: &[u8]) -> io::Result<RoomReactionRemoved> {
        let fb = flatbuffers::root::<fg_room::RoomReactionRemoved>(p).map_err(fb_err)?;
        Ok(RoomReactionRemoved {
            message_id: fb.message_id(),
            room_id: fb.room_id(),
            user_id: fb.user_id(),
            emoji: fb.emoji().ok_or_else(empty_str_err)?.to_string(),
        })
    }

    pub fn enc_room_user_joined(msg: &RoomUserJoined) -> io::Result<Bytes> {
        let mut b = FlatBufferBuilder::with_capacity(128);
        let user = build_user_info(&mut b, &msg.user);
        let root = fg_room::RoomUserJoined::create(
            &mut b,
            &fg_room::RoomUserJoinedArgs {
                room_id: msg.room_id,
                user: Some(user),
                joined_at: msg.joined_at,
            },
        );
        b.finish(root, None);
        Ok(Bytes::copy_from_slice(b.finished_data()))
    }
    pub fn dec_room_user_joined(p: &[u8]) -> io::Result<RoomUserJoined> {
        let fb = flatbuffers::root::<fg_room::RoomUserJoined>(p).map_err(fb_err)?;
        let user = fb.user().ok_or_else(|| {
            IoError::new(ErrorKind::InvalidData, "flatbuffers: user absent")
        })?;
        Ok(RoomUserJoined {
            room_id: fb.room_id(),
            user: decode_user_info(user)?,
            joined_at: fb.joined_at(),
        })
    }

    pub fn enc_room_user_left(msg: &RoomUserLeft) -> io::Result<Bytes> {
        let mut b = FlatBufferBuilder::with_capacity(32);
        let root = fg_room::RoomUserLeft::create(
            &mut b,
            &fg_room::RoomUserLeftArgs {
                room_id: msg.room_id,
                user_id: msg.user_id,
                left_at: msg.left_at,
            },
        );
        b.finish(root, None);
        Ok(Bytes::copy_from_slice(b.finished_data()))
    }
    pub fn dec_room_user_left(p: &[u8]) -> io::Result<RoomUserLeft> {
        let fb = flatbuffers::root::<fg_room::RoomUserLeft>(p).map_err(fb_err)?;
        Ok(RoomUserLeft { room_id: fb.room_id(), user_id: fb.user_id(), left_at: fb.left_at() })
    }

    pub fn enc_room_init(msg: &RoomInit) -> io::Result<Bytes> {
        let mut b = FlatBufferBuilder::with_capacity(1024);
        let name = b.create_string(&msg.name);
        let room_type = b.create_string(&msg.room_type);
        let member_offsets: Vec<_> = msg.members.iter().map(|u| build_user_info(&mut b, u)).collect();
        let members_vec = b.create_vector(&member_offsets);
        let msg_offsets: Vec<_> = msg.recent_messages.iter().map(|m| build_room_message(&mut b, m)).collect();
        let messages_vec = b.create_vector(&msg_offsets);
        let root = fg_room::RoomInit::create(
            &mut b,
            &fg_room::RoomInitArgs {
                room_id: msg.room_id,
                name: Some(name),
                room_type: Some(room_type),
                members: Some(members_vec),
                recent_messages: Some(messages_vec),
            },
        );
        b.finish(root, None);
        Ok(Bytes::copy_from_slice(b.finished_data()))
    }
    pub fn dec_room_init(p: &[u8]) -> io::Result<RoomInit> {
        let fb = flatbuffers::root::<fg_room::RoomInit>(p).map_err(fb_err)?;
        let members = fb
            .members()
            .map(|v| v.iter().map(decode_user_info).collect::<io::Result<Vec<_>>>())
            .transpose()?
            .unwrap_or_default();
        let recent_messages = fb
            .recent_messages()
            .map(|v| v.iter().map(decode_room_message_fb).collect::<io::Result<Vec<_>>>())
            .transpose()?
            .unwrap_or_default();
        Ok(RoomInit {
            room_id: fb.room_id(),
            name: fb.name().ok_or_else(empty_str_err)?.to_string(),
            room_type: fb.room_type().ok_or_else(empty_str_err)?.to_string(),
            members,
            recent_messages,
        })
    }

    pub fn enc_room_close(msg: &RoomClose) -> io::Result<Bytes> {
        let mut b = FlatBufferBuilder::with_capacity(64);
        let reason = b.create_string(&msg.reason);
        let root = fg_room::RoomClose::create(
            &mut b,
            &fg_room::RoomCloseArgs { room_id: msg.room_id, reason: Some(reason) },
        );
        b.finish(root, None);
        Ok(Bytes::copy_from_slice(b.finished_data()))
    }
    pub fn dec_room_close(p: &[u8]) -> io::Result<RoomClose> {
        let fb = flatbuffers::root::<fg_room::RoomClose>(p).map_err(fb_err)?;
        Ok(RoomClose {
            room_id: fb.room_id(),
            reason: fb.reason().ok_or_else(empty_str_err)?.to_string(),
        })
    }

    // -----------------------------------------------------------------------
    // Shard management
    // -----------------------------------------------------------------------

    pub fn enc_shard_assignment(msg: &ShardAssignment) -> io::Result<Bytes> {
        let mut b = FlatBufferBuilder::with_capacity(256);
        let stream_offsets: Vec<_> = msg
            .shard_streams
            .iter()
            .map(|s| {
                let room_ids = b.create_vector(&s.room_ids);
                fg_shard::ShardStreamInfo::create(
                    &mut b,
                    &fg_shard::ShardStreamInfoArgs {
                        shard_id: s.shard_id,
                        room_ids: Some(room_ids),
                    },
                )
            })
            .collect();
        let streams_vec = b.create_vector(&stream_offsets);
        let root = fg_shard::ShardAssignment::create(
            &mut b,
            &fg_shard::ShardAssignmentArgs { shard_streams: Some(streams_vec) },
        );
        b.finish(root, None);
        Ok(Bytes::copy_from_slice(b.finished_data()))
    }
    pub fn dec_shard_assignment(p: &[u8]) -> io::Result<ShardAssignment> {
        let fb = flatbuffers::root::<fg_shard::ShardAssignment>(p).map_err(fb_err)?;
        let shard_streams = fb
            .shard_streams()
            .map(|v| {
                v.iter()
                    .map(|s| {
                        Ok(ShardStreamInfo {
                            shard_id: s.shard_id(),
                            room_ids: s.room_ids().map(|r| r.iter().collect()).unwrap_or_default(),
                        })
                    })
                    .collect::<io::Result<Vec<_>>>()
            })
            .transpose()?
            .unwrap_or_default();
        Ok(ShardAssignment { shard_streams })
    }

    pub fn enc_room_promoted(msg: &RoomPromoted) -> io::Result<Bytes> {
        let mut b = FlatBufferBuilder::with_capacity(32);
        let root = fg_shard::RoomPromoted::create(
            &mut b,
            &fg_shard::RoomPromotedArgs { room_id: msg.room_id, from_shard: msg.from_shard },
        );
        b.finish(root, None);
        Ok(Bytes::copy_from_slice(b.finished_data()))
    }
    pub fn dec_room_promoted(p: &[u8]) -> io::Result<RoomPromoted> {
        let fb = flatbuffers::root::<fg_shard::RoomPromoted>(p).map_err(fb_err)?;
        Ok(RoomPromoted { room_id: fb.room_id(), from_shard: fb.from_shard() })
    }

    pub fn enc_room_demoted(msg: &RoomDemoted) -> io::Result<Bytes> {
        let mut b = FlatBufferBuilder::with_capacity(32);
        let root = fg_shard::RoomDemoted::create(
            &mut b,
            &fg_shard::RoomDemotedArgs { room_id: msg.room_id, to_shard: msg.to_shard },
        );
        b.finish(root, None);
        Ok(Bytes::copy_from_slice(b.finished_data()))
    }
    pub fn dec_room_demoted(p: &[u8]) -> io::Result<RoomDemoted> {
        let fb = flatbuffers::root::<fg_shard::RoomDemoted>(p).map_err(fb_err)?;
        Ok(RoomDemoted { room_id: fb.room_id(), to_shard: fb.to_shard() })
    }

    // -----------------------------------------------------------------------
    // ACK messages
    // -----------------------------------------------------------------------

    pub fn enc_message_delivered(msg: &MessageDelivered) -> io::Result<Bytes> {
        let mut b = FlatBufferBuilder::with_capacity(32);
        let root = fg_ack::MessageDelivered::create(
            &mut b,
            &fg_ack::MessageDeliveredArgs {
                message_id: msg.message_id,
                room_id: msg.room_id,
                delivered_at: msg.delivered_at,
            },
        );
        b.finish(root, None);
        Ok(Bytes::copy_from_slice(b.finished_data()))
    }
    pub fn dec_message_delivered(p: &[u8]) -> io::Result<MessageDelivered> {
        let fb = flatbuffers::root::<fg_ack::MessageDelivered>(p).map_err(fb_err)?;
        Ok(MessageDelivered {
            message_id: fb.message_id(),
            room_id: fb.room_id(),
            delivered_at: fb.delivered_at(),
        })
    }

    pub fn enc_message_read(msg: &MessageRead) -> io::Result<Bytes> {
        let mut b = FlatBufferBuilder::with_capacity(32);
        let root = fg_ack::MessageRead::create(
            &mut b,
            &fg_ack::MessageReadArgs {
                message_id: msg.message_id,
                room_id: msg.room_id,
                read_at: msg.read_at,
            },
        );
        b.finish(root, None);
        Ok(Bytes::copy_from_slice(b.finished_data()))
    }
    pub fn dec_message_read(p: &[u8]) -> io::Result<MessageRead> {
        let fb = flatbuffers::root::<fg_ack::MessageRead>(p).map_err(fb_err)?;
        Ok(MessageRead { message_id: fb.message_id(), room_id: fb.room_id(), read_at: fb.read_at() })
    }

    pub fn enc_message_ack(msg: &MessageAck) -> io::Result<Bytes> {
        let mut b = FlatBufferBuilder::with_capacity(64);
        let nonce = b.create_string(&msg.nonce);
        let root = fg_ack::MessageAck::create(
            &mut b,
            &fg_ack::MessageAckArgs {
                nonce: Some(nonce),
                message_id: msg.message_id,
                timestamp: msg.timestamp,
            },
        );
        b.finish(root, None);
        Ok(Bytes::copy_from_slice(b.finished_data()))
    }
    pub fn dec_message_ack(p: &[u8]) -> io::Result<MessageAck> {
        let fb = flatbuffers::root::<fg_ack::MessageAck>(p).map_err(fb_err)?;
        Ok(MessageAck {
            nonce: fb.nonce().ok_or_else(empty_str_err)?.to_string(),
            message_id: fb.message_id(),
            timestamp: fb.timestamp(),
        })
    }

    // -----------------------------------------------------------------------
    // Upload messages
    // -----------------------------------------------------------------------

    pub fn enc_upload_start(msg: &UploadStart) -> io::Result<Bytes> {
        let mut b = FlatBufferBuilder::with_capacity(128);
        let filename = b.create_string(&msg.filename);
        let mime_type = b.create_string(&msg.mime_type);
        let checksum = b.create_string(&msg.checksum);
        let root = fg_upload::UploadStart::create(
            &mut b,
            &fg_upload::UploadStartArgs {
                room_id: msg.room_id,
                filename: Some(filename),
                mime_type: Some(mime_type),
                size_: msg.size,
                checksum: Some(checksum),
            },
        );
        b.finish(root, None);
        Ok(Bytes::copy_from_slice(b.finished_data()))
    }
    pub fn dec_upload_start(p: &[u8]) -> io::Result<UploadStart> {
        let fb = flatbuffers::root::<fg_upload::UploadStart>(p).map_err(fb_err)?;
        Ok(UploadStart {
            room_id: fb.room_id(),
            filename: fb.filename().ok_or_else(empty_str_err)?.to_string(),
            mime_type: fb.mime_type().ok_or_else(empty_str_err)?.to_string(),
            size: fb.size_(),
            checksum: fb.checksum().ok_or_else(empty_str_err)?.to_string(),
        })
    }

    pub fn enc_upload_chunk(msg: &UploadChunk) -> io::Result<Bytes> {
        let mut b = FlatBufferBuilder::with_capacity(32);
        let root = fg_upload::UploadChunk::create(
            &mut b,
            &fg_upload::UploadChunkArgs {
                upload_id: msg.upload_id,
                offset: msg.offset,
                size_: msg.size,
            },
        );
        b.finish(root, None);
        Ok(Bytes::copy_from_slice(b.finished_data()))
    }
    pub fn dec_upload_chunk(p: &[u8]) -> io::Result<UploadChunk> {
        let fb = flatbuffers::root::<fg_upload::UploadChunk>(p).map_err(fb_err)?;
        Ok(UploadChunk { upload_id: fb.upload_id(), offset: fb.offset(), size: fb.size_() })
    }

    pub fn enc_upload_complete(msg: &UploadComplete) -> io::Result<Bytes> {
        let mut b = FlatBufferBuilder::with_capacity(32);
        let root = fg_upload::UploadComplete::create(
            &mut b,
            &fg_upload::UploadCompleteArgs { upload_id: msg.upload_id },
        );
        b.finish(root, None);
        Ok(Bytes::copy_from_slice(b.finished_data()))
    }
    pub fn dec_upload_complete(p: &[u8]) -> io::Result<UploadComplete> {
        let fb = flatbuffers::root::<fg_upload::UploadComplete>(p).map_err(fb_err)?;
        Ok(UploadComplete { upload_id: fb.upload_id() })
    }

    pub fn enc_upload_cancel(msg: &UploadCancel) -> io::Result<Bytes> {
        let mut b = FlatBufferBuilder::with_capacity(64);
        let reason = b.create_string(&msg.reason);
        let root = fg_upload::UploadCancel::create(
            &mut b,
            &fg_upload::UploadCancelArgs { upload_id: msg.upload_id, reason: Some(reason) },
        );
        b.finish(root, None);
        Ok(Bytes::copy_from_slice(b.finished_data()))
    }
    pub fn dec_upload_cancel(p: &[u8]) -> io::Result<UploadCancel> {
        let fb = flatbuffers::root::<fg_upload::UploadCancel>(p).map_err(fb_err)?;
        Ok(UploadCancel {
            upload_id: fb.upload_id(),
            reason: fb.reason().ok_or_else(empty_str_err)?.to_string(),
        })
    }

    pub fn enc_upload_ack(msg: &UploadAck) -> io::Result<Bytes> {
        let mut b = FlatBufferBuilder::with_capacity(64);
        let url = msg.url.as_deref().map(|s| b.create_string(s));
        let root = fg_upload::UploadAck::create(
            &mut b,
            &fg_upload::UploadAckArgs {
                upload_id: msg.upload_id,
                bytes_received: msg.bytes_received,
                has_url: msg.url.is_some(),
                url,
            },
        );
        b.finish(root, None);
        Ok(Bytes::copy_from_slice(b.finished_data()))
    }
    pub fn dec_upload_ack(p: &[u8]) -> io::Result<UploadAck> {
        let fb = flatbuffers::root::<fg_upload::UploadAck>(p).map_err(fb_err)?;
        Ok(UploadAck {
            upload_id: fb.upload_id(),
            bytes_received: fb.bytes_received(),
            url: if fb.has_url() { fb.url().map(|s| s.to_string()) } else { None },
        })
    }

    // -----------------------------------------------------------------------
    // Presence / datagram messages
    // -----------------------------------------------------------------------

    pub fn enc_typing(msg: &Typing) -> io::Result<Bytes> {
        let mut b = FlatBufferBuilder::with_capacity(32);
        let root = fg_presence::Typing::create(
            &mut b,
            &fg_presence::TypingArgs {
                room_id: msg.room_id,
                has_user_id: msg.user_id.is_some(),
                user_id: msg.user_id.unwrap_or(0),
            },
        );
        b.finish(root, None);
        Ok(Bytes::copy_from_slice(b.finished_data()))
    }
    pub fn dec_typing(p: &[u8]) -> io::Result<Typing> {
        let fb = flatbuffers::root::<fg_presence::Typing>(p).map_err(fb_err)?;
        Ok(Typing {
            room_id: fb.room_id(),
            user_id: if fb.has_user_id() { Some(fb.user_id()) } else { None },
        })
    }

    pub fn enc_stop_typing(msg: &StopTyping) -> io::Result<Bytes> {
        let mut b = FlatBufferBuilder::with_capacity(32);
        let root = fg_presence::StopTyping::create(
            &mut b,
            &fg_presence::StopTypingArgs {
                room_id: msg.room_id,
                has_user_id: msg.user_id.is_some(),
                user_id: msg.user_id.unwrap_or(0),
            },
        );
        b.finish(root, None);
        Ok(Bytes::copy_from_slice(b.finished_data()))
    }
    pub fn dec_stop_typing(p: &[u8]) -> io::Result<StopTyping> {
        let fb = flatbuffers::root::<fg_presence::StopTyping>(p).map_err(fb_err)?;
        Ok(StopTyping {
            room_id: fb.room_id(),
            user_id: if fb.has_user_id() { Some(fb.user_id()) } else { None },
        })
    }

    pub fn enc_presence_online(msg: &PresenceOnline) -> io::Result<Bytes> {
        let mut b = FlatBufferBuilder::with_capacity(64);
        let status = msg.status.as_deref().map(|s| b.create_string(s));
        let root = fg_presence::PresenceOnline::create(
            &mut b,
            &fg_presence::PresenceOnlineArgs {
                user_id: msg.user_id,
                has_status: msg.status.is_some(),
                status,
            },
        );
        b.finish(root, None);
        Ok(Bytes::copy_from_slice(b.finished_data()))
    }
    pub fn dec_presence_online(p: &[u8]) -> io::Result<PresenceOnline> {
        let fb = flatbuffers::root::<fg_presence::PresenceOnline>(p).map_err(fb_err)?;
        Ok(PresenceOnline {
            user_id: fb.user_id(),
            status: if fb.has_status() { fb.status().map(|s| s.to_string()) } else { None },
        })
    }

    pub fn enc_presence_offline(msg: &PresenceOffline) -> io::Result<Bytes> {
        let mut b = FlatBufferBuilder::with_capacity(32);
        let root = fg_presence::PresenceOffline::create(
            &mut b,
            &fg_presence::PresenceOfflineArgs {
                user_id: msg.user_id,
                last_seen: msg.last_seen,
            },
        );
        b.finish(root, None);
        Ok(Bytes::copy_from_slice(b.finished_data()))
    }
    pub fn dec_presence_offline(p: &[u8]) -> io::Result<PresenceOffline> {
        let fb = flatbuffers::root::<fg_presence::PresenceOffline>(p).map_err(fb_err)?;
        Ok(PresenceOffline { user_id: fb.user_id(), last_seen: fb.last_seen() })
    }

    pub fn enc_presence_away(msg: &PresenceAway) -> io::Result<Bytes> {
        let mut b = FlatBufferBuilder::with_capacity(32);
        let root = fg_presence::PresenceAway::create(
            &mut b,
            &fg_presence::PresenceAwayArgs {
                user_id: msg.user_id,
                away_since: msg.away_since,
            },
        );
        b.finish(root, None);
        Ok(Bytes::copy_from_slice(b.finished_data()))
    }
    pub fn dec_presence_away(p: &[u8]) -> io::Result<PresenceAway> {
        let fb = flatbuffers::root::<fg_presence::PresenceAway>(p).map_err(fb_err)?;
        Ok(PresenceAway { user_id: fb.user_id(), away_since: fb.away_since() })
    }

    // -----------------------------------------------------------------------
    // Error message
    // -----------------------------------------------------------------------

    pub fn enc_error(msg: &Error) -> io::Result<Bytes> {
        let mut b = FlatBufferBuilder::with_capacity(64);
        let message = b.create_string(&msg.message);
        let context = msg.context.as_deref().map(|s| b.create_string(s));
        let root = fg_err::Error::create(
            &mut b,
            &fg_err::ErrorArgs {
                code: msg.code,
                message: Some(message),
                has_context: msg.context.is_some(),
                context,
            },
        );
        b.finish(root, None);
        Ok(Bytes::copy_from_slice(b.finished_data()))
    }
    pub fn dec_error(p: &[u8]) -> io::Result<Error> {
        let fb = flatbuffers::root::<fg_err::Error>(p).map_err(fb_err)?;
        Ok(Error {
            code: fb.code(),
            message: fb.message().ok_or_else(empty_str_err)?.to_string(),
            context: if fb.has_context() { fb.context().map(|s| s.to_string()) } else { None },
        })
    }
} // mod fb

// ---------------------------------------------------------------------------
// Trait impls — FlatBuffers backend
// ---------------------------------------------------------------------------

macro_rules! impl_fb_codec {
    ($type:ty, $frame_type:expr, $enc:expr, $dec:expr) => {
        #[cfg(not(feature = "debug-json"))]
        impl Encodable for $type {
            fn frame_type(&self) -> FrameType { $frame_type }
            fn encode_payload(&self) -> io::Result<Bytes> { $enc(self) }
        }
        #[cfg(not(feature = "debug-json"))]
        impl Decodable for $type {
            fn expected_frame_type() -> FrameType { $frame_type }
            fn decode_payload(payload: &[u8]) -> io::Result<Self> { $dec(payload) }
        }
    };
}

impl_fb_codec!(Hello,              FrameType::Hello,              fb::enc_hello,               fb::dec_hello);
impl_fb_codec!(HelloAck,           FrameType::HelloAck,           fb::enc_hello_ack,           fb::dec_hello_ack);
impl_fb_codec!(Auth,               FrameType::Auth,               fb::enc_auth,                fb::dec_auth);
impl_fb_codec!(AuthOk,             FrameType::AuthOk,             fb::enc_auth_ok,             fb::dec_auth_ok);
impl_fb_codec!(AuthFailed,         FrameType::AuthFailed,         fb::enc_auth_failed,         fb::dec_auth_failed);
impl_fb_codec!(Ping,               FrameType::Ping,               fb::enc_ping,                fb::dec_ping);
impl_fb_codec!(Pong,               FrameType::Pong,               fb::enc_pong,                fb::dec_pong);
impl_fb_codec!(Throttle,           FrameType::Throttle,           fb::enc_throttle,            fb::dec_throttle);
impl_fb_codec!(Goodbye,            FrameType::Goodbye,            fb::enc_goodbye,             fb::dec_goodbye);
impl_fb_codec!(ServerCommand,      FrameType::ServerCommand,      fb::enc_server_command,      fb::dec_server_command);
impl_fb_codec!(SendMessage,        FrameType::SendMessage,        fb::enc_send_message,        fb::dec_send_message);
impl_fb_codec!(EditMessage,        FrameType::EditMessage,        fb::enc_edit_message,        fb::dec_edit_message);
impl_fb_codec!(DeleteMessage,      FrameType::DeleteMessage,      fb::enc_delete_message,      fb::dec_delete_message);
impl_fb_codec!(AddReaction,        FrameType::AddReaction,        fb::enc_add_reaction,        fb::dec_add_reaction);
impl_fb_codec!(RemoveReaction,     FrameType::RemoveReaction,     fb::enc_remove_reaction,     fb::dec_remove_reaction);
impl_fb_codec!(JoinRoom,           FrameType::JoinRoom,           fb::enc_join_room,           fb::dec_join_room);
impl_fb_codec!(LeaveRoom,          FrameType::LeaveRoom,          fb::enc_leave_room,          fb::dec_leave_room);
impl_fb_codec!(CreateRoom,         FrameType::CreateRoom,         fb::enc_create_room,         fb::dec_create_room);
impl_fb_codec!(RoomMessage,        FrameType::RoomMessage,        fb::enc_room_message,        fb::dec_room_message);
impl_fb_codec!(RoomMessageEdited,  FrameType::RoomMessageEdited,  fb::enc_room_message_edited, fb::dec_room_message_edited);
impl_fb_codec!(RoomMessageDeleted, FrameType::RoomMessageDeleted, fb::enc_room_message_deleted,fb::dec_room_message_deleted);
impl_fb_codec!(RoomReactionAdded,  FrameType::RoomReactionAdded,  fb::enc_room_reaction_added, fb::dec_room_reaction_added);
impl_fb_codec!(RoomReactionRemoved,FrameType::RoomReactionRemoved,fb::enc_room_reaction_removed,fb::dec_room_reaction_removed);
impl_fb_codec!(RoomUserJoined,     FrameType::RoomUserJoined,     fb::enc_room_user_joined,    fb::dec_room_user_joined);
impl_fb_codec!(RoomUserLeft,       FrameType::RoomUserLeft,       fb::enc_room_user_left,      fb::dec_room_user_left);
impl_fb_codec!(RoomInit,           FrameType::RoomInit,           fb::enc_room_init,           fb::dec_room_init);
impl_fb_codec!(RoomClose,          FrameType::RoomClose,          fb::enc_room_close,          fb::dec_room_close);
impl_fb_codec!(ShardAssignment,    FrameType::ShardAssignment,    fb::enc_shard_assignment,    fb::dec_shard_assignment);
impl_fb_codec!(RoomPromoted,       FrameType::RoomPromoted,       fb::enc_room_promoted,       fb::dec_room_promoted);
impl_fb_codec!(RoomDemoted,        FrameType::RoomDemoted,        fb::enc_room_demoted,        fb::dec_room_demoted);
impl_fb_codec!(MessageDelivered,   FrameType::MessageDelivered,   fb::enc_message_delivered,   fb::dec_message_delivered);
impl_fb_codec!(MessageRead,        FrameType::MessageRead,        fb::enc_message_read,        fb::dec_message_read);
impl_fb_codec!(MessageAck,         FrameType::MessageAck,         fb::enc_message_ack,         fb::dec_message_ack);
impl_fb_codec!(UploadStart,        FrameType::UploadStart,        fb::enc_upload_start,        fb::dec_upload_start);
impl_fb_codec!(UploadChunk,        FrameType::UploadChunk,        fb::enc_upload_chunk,        fb::dec_upload_chunk);
impl_fb_codec!(UploadComplete,     FrameType::UploadComplete,     fb::enc_upload_complete,     fb::dec_upload_complete);
impl_fb_codec!(UploadCancel,       FrameType::UploadCancel,       fb::enc_upload_cancel,       fb::dec_upload_cancel);
impl_fb_codec!(UploadAck,          FrameType::UploadAck,          fb::enc_upload_ack,          fb::dec_upload_ack);
impl_fb_codec!(Typing,             FrameType::Typing,             fb::enc_typing,              fb::dec_typing);
impl_fb_codec!(StopTyping,         FrameType::StopTyping,         fb::enc_stop_typing,         fb::dec_stop_typing);
impl_fb_codec!(PresenceOnline,     FrameType::PresenceOnline,     fb::enc_presence_online,     fb::dec_presence_online);
impl_fb_codec!(PresenceOffline,    FrameType::PresenceOffline,    fb::enc_presence_offline,    fb::dec_presence_offline);
impl_fb_codec!(PresenceAway,       FrameType::PresenceAway,       fb::enc_presence_away,       fb::dec_presence_away);
impl_fb_codec!(Error,              FrameType::Error,              fb::enc_error,               fb::dec_error);

// ---------------------------------------------------------------------------
// DecodedMessage enum — mode-independent
// ---------------------------------------------------------------------------

/// Decode any frame into a typed message enum.
#[derive(Debug, Clone)]
pub enum DecodedMessage {
    Hello(Hello), HelloAck(HelloAck), Auth(Auth), AuthOk(AuthOk), AuthFailed(AuthFailed),
    Ping(Ping), Pong(Pong), Throttle(Throttle), Goodbye(Goodbye), ServerCommand(ServerCommand),
    SendMessage(SendMessage), EditMessage(EditMessage), DeleteMessage(DeleteMessage),
    AddReaction(AddReaction), RemoveReaction(RemoveReaction), JoinRoom(JoinRoom),
    LeaveRoom(LeaveRoom), CreateRoom(CreateRoom),
    RoomMessage(RoomMessage), RoomMessageEdited(RoomMessageEdited),
    RoomMessageDeleted(RoomMessageDeleted), RoomReactionAdded(RoomReactionAdded),
    RoomReactionRemoved(RoomReactionRemoved), RoomUserJoined(RoomUserJoined),
    RoomUserLeft(RoomUserLeft), RoomInit(RoomInit), RoomClose(RoomClose),
    ShardAssignment(ShardAssignment), RoomPromoted(RoomPromoted), RoomDemoted(RoomDemoted),
    MessageDelivered(MessageDelivered), MessageRead(MessageRead), MessageAck(MessageAck),
    UploadStart(UploadStart), UploadChunk(UploadChunk), UploadComplete(UploadComplete),
    UploadCancel(UploadCancel), UploadAck(UploadAck),
    Typing(Typing), StopTyping(StopTyping), PresenceOnline(PresenceOnline),
    PresenceOffline(PresenceOffline), PresenceAway(PresenceAway),
    Error(Error),
}

impl DecodedMessage {
    pub fn decode(frame: &Frame) -> io::Result<Self> {
        let p = &frame.payload;
        match frame.frame_type {
            FrameType::Hello           => Ok(Self::Hello(Hello::decode_payload(p)?)),
            FrameType::HelloAck        => Ok(Self::HelloAck(HelloAck::decode_payload(p)?)),
            FrameType::Auth            => Ok(Self::Auth(Auth::decode_payload(p)?)),
            FrameType::AuthOk          => Ok(Self::AuthOk(AuthOk::decode_payload(p)?)),
            FrameType::AuthFailed      => Ok(Self::AuthFailed(AuthFailed::decode_payload(p)?)),
            FrameType::Ping            => Ok(Self::Ping(Ping::decode_payload(p)?)),
            FrameType::Pong            => Ok(Self::Pong(Pong::decode_payload(p)?)),
            FrameType::Throttle        => Ok(Self::Throttle(Throttle::decode_payload(p)?)),
            FrameType::Goodbye         => Ok(Self::Goodbye(Goodbye::decode_payload(p)?)),
            FrameType::ServerCommand   => Ok(Self::ServerCommand(ServerCommand::decode_payload(p)?)),
            FrameType::SendMessage     => Ok(Self::SendMessage(SendMessage::decode_payload(p)?)),
            FrameType::EditMessage     => Ok(Self::EditMessage(EditMessage::decode_payload(p)?)),
            FrameType::DeleteMessage   => Ok(Self::DeleteMessage(DeleteMessage::decode_payload(p)?)),
            FrameType::AddReaction     => Ok(Self::AddReaction(AddReaction::decode_payload(p)?)),
            FrameType::RemoveReaction  => Ok(Self::RemoveReaction(RemoveReaction::decode_payload(p)?)),
            FrameType::JoinRoom        => Ok(Self::JoinRoom(JoinRoom::decode_payload(p)?)),
            FrameType::LeaveRoom       => Ok(Self::LeaveRoom(LeaveRoom::decode_payload(p)?)),
            FrameType::CreateRoom      => Ok(Self::CreateRoom(CreateRoom::decode_payload(p)?)),
            FrameType::RoomMessage     => Ok(Self::RoomMessage(RoomMessage::decode_payload(p)?)),
            FrameType::RoomMessageEdited    => Ok(Self::RoomMessageEdited(RoomMessageEdited::decode_payload(p)?)),
            FrameType::RoomMessageDeleted   => Ok(Self::RoomMessageDeleted(RoomMessageDeleted::decode_payload(p)?)),
            FrameType::RoomReactionAdded    => Ok(Self::RoomReactionAdded(RoomReactionAdded::decode_payload(p)?)),
            FrameType::RoomReactionRemoved  => Ok(Self::RoomReactionRemoved(RoomReactionRemoved::decode_payload(p)?)),
            FrameType::RoomUserJoined  => Ok(Self::RoomUserJoined(RoomUserJoined::decode_payload(p)?)),
            FrameType::RoomUserLeft    => Ok(Self::RoomUserLeft(RoomUserLeft::decode_payload(p)?)),
            FrameType::RoomInit        => Ok(Self::RoomInit(RoomInit::decode_payload(p)?)),
            FrameType::RoomClose       => Ok(Self::RoomClose(RoomClose::decode_payload(p)?)),
            FrameType::ShardAssignment => Ok(Self::ShardAssignment(ShardAssignment::decode_payload(p)?)),
            FrameType::RoomPromoted    => Ok(Self::RoomPromoted(RoomPromoted::decode_payload(p)?)),
            FrameType::RoomDemoted     => Ok(Self::RoomDemoted(RoomDemoted::decode_payload(p)?)),
            FrameType::MessageDelivered=> Ok(Self::MessageDelivered(MessageDelivered::decode_payload(p)?)),
            FrameType::MessageRead     => Ok(Self::MessageRead(MessageRead::decode_payload(p)?)),
            FrameType::MessageAck      => Ok(Self::MessageAck(MessageAck::decode_payload(p)?)),
            FrameType::UploadStart     => Ok(Self::UploadStart(UploadStart::decode_payload(p)?)),
            FrameType::UploadChunk     => Ok(Self::UploadChunk(UploadChunk::decode_payload(p)?)),
            FrameType::UploadComplete  => Ok(Self::UploadComplete(UploadComplete::decode_payload(p)?)),
            FrameType::UploadCancel    => Ok(Self::UploadCancel(UploadCancel::decode_payload(p)?)),
            FrameType::UploadAck       => Ok(Self::UploadAck(UploadAck::decode_payload(p)?)),
            FrameType::Typing          => Ok(Self::Typing(Typing::decode_payload(p)?)),
            FrameType::StopTyping      => Ok(Self::StopTyping(StopTyping::decode_payload(p)?)),
            FrameType::PresenceOnline  => Ok(Self::PresenceOnline(PresenceOnline::decode_payload(p)?)),
            FrameType::PresenceOffline => Ok(Self::PresenceOffline(PresenceOffline::decode_payload(p)?)),
            FrameType::PresenceAway    => Ok(Self::PresenceAway(PresenceAway::decode_payload(p)?)),
            FrameType::Error           => Ok(Self::Error(Error::decode_payload(p)?)),
        }
    }

    pub fn frame_type(&self) -> FrameType {
        match self {
            Self::Hello(_) => FrameType::Hello,
            Self::HelloAck(_) => FrameType::HelloAck,
            Self::Auth(_) => FrameType::Auth,
            Self::AuthOk(_) => FrameType::AuthOk,
            Self::AuthFailed(_) => FrameType::AuthFailed,
            Self::Ping(_) => FrameType::Ping,
            Self::Pong(_) => FrameType::Pong,
            Self::Throttle(_) => FrameType::Throttle,
            Self::Goodbye(_) => FrameType::Goodbye,
            Self::ServerCommand(_) => FrameType::ServerCommand,
            Self::SendMessage(_) => FrameType::SendMessage,
            Self::EditMessage(_) => FrameType::EditMessage,
            Self::DeleteMessage(_) => FrameType::DeleteMessage,
            Self::AddReaction(_) => FrameType::AddReaction,
            Self::RemoveReaction(_) => FrameType::RemoveReaction,
            Self::JoinRoom(_) => FrameType::JoinRoom,
            Self::LeaveRoom(_) => FrameType::LeaveRoom,
            Self::CreateRoom(_) => FrameType::CreateRoom,
            Self::RoomMessage(_) => FrameType::RoomMessage,
            Self::RoomMessageEdited(_) => FrameType::RoomMessageEdited,
            Self::RoomMessageDeleted(_) => FrameType::RoomMessageDeleted,
            Self::RoomReactionAdded(_) => FrameType::RoomReactionAdded,
            Self::RoomReactionRemoved(_) => FrameType::RoomReactionRemoved,
            Self::RoomUserJoined(_) => FrameType::RoomUserJoined,
            Self::RoomUserLeft(_) => FrameType::RoomUserLeft,
            Self::RoomInit(_) => FrameType::RoomInit,
            Self::RoomClose(_) => FrameType::RoomClose,
            Self::ShardAssignment(_) => FrameType::ShardAssignment,
            Self::RoomPromoted(_) => FrameType::RoomPromoted,
            Self::RoomDemoted(_) => FrameType::RoomDemoted,
            Self::MessageDelivered(_) => FrameType::MessageDelivered,
            Self::MessageRead(_) => FrameType::MessageRead,
            Self::MessageAck(_) => FrameType::MessageAck,
            Self::UploadStart(_) => FrameType::UploadStart,
            Self::UploadChunk(_) => FrameType::UploadChunk,
            Self::UploadComplete(_) => FrameType::UploadComplete,
            Self::UploadCancel(_) => FrameType::UploadCancel,
            Self::UploadAck(_) => FrameType::UploadAck,
            Self::Typing(_) => FrameType::Typing,
            Self::StopTyping(_) => FrameType::StopTyping,
            Self::PresenceOnline(_) => FrameType::PresenceOnline,
            Self::PresenceOffline(_) => FrameType::PresenceOffline,
            Self::PresenceAway(_) => FrameType::PresenceAway,
            Self::Error(_) => FrameType::Error,
        }
    }

    pub fn is_control(&self) -> bool { self.frame_type().is_control() }
    pub fn is_chat_command(&self) -> bool { self.frame_type().is_chat_command() }
    pub fn is_room_message(&self) -> bool { self.frame_type().is_room_message() }
    pub fn is_datagram(&self) -> bool { self.frame_type().is_datagram() }
}

/// Encode a message to framed bytes.
pub fn encode<T: Encodable>(msg: &T) -> io::Result<Bytes> {
    msg.encode_frame().map(|f| f.encode_to_bytes())
}

/// Decode a frame to a specific message type.
pub fn decode<T: Decodable>(frame: &Frame) -> io::Result<T> {
    T::decode_frame(frame)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_roundtrip() {
        let original = SendMessage {
            room_id: 123,
            content: "Hello, World!".to_string(),
            nonce: "abc123".to_string(),
            reply_to: None,
            attachments: vec![],
        };
        let frame = original.encode_frame().unwrap();
        assert_eq!(frame.frame_type, FrameType::SendMessage);
        let decoded = SendMessage::decode_frame(&frame).unwrap();
        assert_eq!(original.room_id, decoded.room_id);
        assert_eq!(original.content, decoded.content);
        assert_eq!(original.nonce, decoded.nonce);
        assert_eq!(original.reply_to, decoded.reply_to);
    }

    #[test]
    fn test_decoded_message_enum() {
        let msg = Ping { timestamp: 12345 };
        let frame = msg.encode_frame().unwrap();
        let decoded = DecodedMessage::decode(&frame).unwrap();
        assert!(decoded.is_control());
        match decoded {
            DecodedMessage::Ping(ping) => assert_eq!(ping.timestamp, 12345),
            _ => panic!("Expected Ping"),
        }
    }

    #[test]
    fn test_wrong_frame_type() {
        let msg = Ping { timestamp: 12345 };
        let frame = msg.encode_frame().unwrap();
        assert!(Pong::decode_frame(&frame).is_err());
    }

    #[test]
    fn test_encode_helper() {
        let msg = Hello::default();
        let bytes = encode(&msg).unwrap();
        assert!(!bytes.is_empty());
    }

    #[test]
    fn test_error_message_encoding() {
        let err = Error::auth_failed("Invalid credentials");
        let frame = err.encode_frame().unwrap();
        let decoded = Error::decode_frame(&frame).unwrap();
        assert_eq!(decoded.code, Error::AUTH_FAILED);
        assert_eq!(decoded.message, "Invalid credentials");
    }

    #[test]
    fn test_room_message_encoding() {
        let msg = RoomMessage {
            message_id: 456,
            room_id: 123,
            sender: UserInfo { user_id: 1, username: "alice".to_string(), avatar_url: None },
            content: "Test message".to_string(),
            timestamp: 1234567890,
            reply_to: Some(100),
            attachments: vec!["https://example.com/file.png".to_string()],
            nonce: Some("nonce123".to_string()),
        };
        let frame = msg.encode_frame().unwrap();
        let decoded = RoomMessage::decode_frame(&frame).unwrap();
        assert_eq!(decoded.message_id, 456);
        assert_eq!(decoded.reply_to, Some(100));
        assert_eq!(decoded.attachments.len(), 1);
        assert_eq!(decoded.sender.username, "alice");
    }

    #[test]
    fn test_optional_fields_roundtrip() {
        // Throttle with room_id present
        let t = Throttle { room_id: Some(42), delay_ms: 500, reason: "test".to_string() };
        let frame = t.encode_frame().unwrap();
        let d = Throttle::decode_frame(&frame).unwrap();
        assert_eq!(d.room_id, Some(42));
        assert_eq!(d.delay_ms, 500);

        // Throttle without room_id
        let t2 = Throttle { room_id: None, delay_ms: 100, reason: "global".to_string() };
        let frame2 = t2.encode_frame().unwrap();
        let d2 = Throttle::decode_frame(&frame2).unwrap();
        assert_eq!(d2.room_id, None);

        // Typing with user_id
        let ty = Typing { room_id: 7, user_id: Some(99) };
        let frame3 = ty.encode_frame().unwrap();
        let d3 = Typing::decode_frame(&frame3).unwrap();
        assert_eq!(d3.user_id, Some(99));
    }

    #[test]
    fn test_room_init_with_nested_vecs() {
        let msg = RoomInit {
            room_id: 1,
            name: "general".to_string(),
            room_type: "public".to_string(),
            members: vec![
                UserInfo { user_id: 1, username: "alice".to_string(), avatar_url: None },
                UserInfo {
                    user_id: 2,
                    username: "bob".to_string(),
                    avatar_url: Some("https://example.com/bob.png".to_string()),
                },
            ],
            recent_messages: vec![RoomMessage {
                message_id: 10,
                room_id: 1,
                sender: UserInfo { user_id: 1, username: "alice".to_string(), avatar_url: None },
                content: "hi".to_string(),
                timestamp: 1000,
                reply_to: None,
                attachments: vec![],
                nonce: None,
            }],
        };
        let frame = msg.encode_frame().unwrap();
        let decoded = RoomInit::decode_frame(&frame).unwrap();
        assert_eq!(decoded.members.len(), 2);
        assert_eq!(decoded.members[1].username, "bob");
        assert_eq!(decoded.members[1].avatar_url, Some("https://example.com/bob.png".to_string()));
        assert_eq!(decoded.recent_messages.len(), 1);
        assert_eq!(decoded.recent_messages[0].content, "hi");
    }

    #[test]
    fn test_shard_assignment_roundtrip() {
        let msg = ShardAssignment {
            shard_streams: vec![
                ShardStreamInfo { shard_id: 0, room_ids: vec![1, 2, 3] },
                ShardStreamInfo { shard_id: 1, room_ids: vec![4, 5] },
            ],
        };
        let frame = msg.encode_frame().unwrap();
        let decoded = ShardAssignment::decode_frame(&frame).unwrap();
        assert_eq!(decoded.shard_streams.len(), 2);
        assert_eq!(decoded.shard_streams[0].room_ids, vec![1, 2, 3]);
    }

    #[test]
    fn test_server_command_json_params() {
        let msg = ServerCommand {
            command: "ban".to_string(),
            params: serde_json::json!({ "user_id": 42, "reason": "spam" }),
        };
        let frame = msg.encode_frame().unwrap();
        let decoded = ServerCommand::decode_frame(&frame).unwrap();
        assert_eq!(decoded.command, "ban");
        assert_eq!(decoded.params["user_id"], 42);
    }
}
