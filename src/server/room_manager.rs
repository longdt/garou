//! Room management for the chat server
//!
//! This module handles server-side room state, member tracking, and room lifecycle.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::RwLock;

use crate::current_timestamp;
use crate::protocol::messages::{RoomId, RoomMessage, UserId, UserInfo};

/// Room type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoomType {
    /// Direct message between two users
    Direct,
    /// Group chat with multiple members
    Group,
    /// Public channel anyone can join
    Channel,
}

impl Default for RoomType {
    fn default() -> Self {
        RoomType::Group
    }
}

/// A member of a room
#[derive(Debug, Clone)]
pub struct RoomMember {
    /// User ID
    pub user_id: UserId,
    /// Username
    pub username: String,
    /// Avatar URL (optional)
    pub avatar_url: Option<String>,
    /// When the user joined this room
    pub joined_at: u64,
    /// User's role in the room
    pub role: MemberRole,
    /// Last time this user was active in this room
    pub last_activity: u64,
}

impl RoomMember {
    pub fn new(user_id: UserId, username: String) -> Self {
        let now = current_timestamp();
        Self {
            user_id,
            username,
            avatar_url: None,
            joined_at: now,
            role: MemberRole::Member,
            last_activity: now,
        }
    }

    pub fn with_role(mut self, role: MemberRole) -> Self {
        self.role = role;
        self
    }

    pub fn to_user_info(&self) -> UserInfo {
        UserInfo {
            user_id: self.user_id,
            username: self.username.clone(),
            avatar_url: self.avatar_url.clone(),
        }
    }
}

/// Member role within a room
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberRole {
    /// Room owner (can delete room, manage admins)
    Owner,
    /// Room admin (can manage members, settings)
    Admin,
    /// Moderator (can mute/kick members)
    Moderator,
    /// Regular member
    Member,
    /// Guest with limited permissions
    Guest,
}

impl MemberRole {
    pub fn can_manage_members(&self) -> bool {
        matches!(self, MemberRole::Owner | MemberRole::Admin)
    }

    pub fn can_moderate(&self) -> bool {
        matches!(
            self,
            MemberRole::Owner | MemberRole::Admin | MemberRole::Moderator
        )
    }

    pub fn can_send_messages(&self) -> bool {
        !matches!(self, MemberRole::Guest)
    }
}

/// A chat room
#[derive(Debug)]
pub struct Room {
    /// Room ID
    pub id: RoomId,
    /// Room name
    pub name: String,
    /// Room type
    pub room_type: RoomType,
    /// Room members indexed by user ID
    members: RwLock<HashMap<UserId, RoomMember>>,
    /// Recent messages (circular buffer, configurable size)
    recent_messages: RwLock<Vec<RoomMessage>>,
    /// Maximum recent messages to keep
    max_recent_messages: usize,
    /// Room creation timestamp
    pub created_at: u64,
    /// Last message timestamp
    pub last_message_at: RwLock<u64>,
    /// Total message count
    pub message_count: RwLock<u64>,
    /// Room metadata/settings
    pub metadata: RwLock<HashMap<String, String>>,
}

impl Room {
    /// Create a new room
    pub fn new(id: RoomId, name: String, room_type: RoomType) -> Self {
        let now = current_timestamp();
        Self {
            id,
            name,
            room_type,
            members: RwLock::new(HashMap::new()),
            recent_messages: RwLock::new(Vec::new()),
            max_recent_messages: 100,
            created_at: now,
            last_message_at: RwLock::new(now),
            message_count: RwLock::new(0),
            metadata: RwLock::new(HashMap::new()),
        }
    }

    /// Create a new room with custom recent message limit
    pub fn with_recent_limit(mut self, limit: usize) -> Self {
        self.max_recent_messages = limit;
        self
    }

    /// Add a member to the room
    pub async fn add_member(&self, member: RoomMember) -> bool {
        let mut members = self.members.write().await;
        if members.contains_key(&member.user_id) {
            return false;
        }
        members.insert(member.user_id, member);
        true
    }

    /// Remove a member from the room
    pub async fn remove_member(&self, user_id: UserId) -> Option<RoomMember> {
        let mut members = self.members.write().await;
        members.remove(&user_id)
    }

    /// Check if a user is a member
    pub async fn is_member(&self, user_id: UserId) -> bool {
        let members = self.members.read().await;
        members.contains_key(&user_id)
    }

    /// Get a member by user ID
    pub async fn get_member(&self, user_id: UserId) -> Option<RoomMember> {
        let members = self.members.read().await;
        members.get(&user_id).cloned()
    }

    /// Get all members
    pub async fn get_members(&self) -> Vec<RoomMember> {
        let members = self.members.read().await;
        members.values().cloned().collect()
    }

    /// Get member count
    pub async fn member_count(&self) -> usize {
        let members = self.members.read().await;
        members.len()
    }

    /// Get all member user IDs
    pub async fn get_member_ids(&self) -> Vec<UserId> {
        let members = self.members.read().await;
        members.keys().copied().collect()
    }

    /// Update member's last activity
    pub async fn touch_member(&self, user_id: UserId) {
        let mut members = self.members.write().await;
        if let Some(member) = members.get_mut(&user_id) {
            member.last_activity = current_timestamp();
        }
    }

    /// Update member's role
    pub async fn set_member_role(&self, user_id: UserId, role: MemberRole) -> bool {
        let mut members = self.members.write().await;
        if let Some(member) = members.get_mut(&user_id) {
            member.role = role;
            true
        } else {
            false
        }
    }

    /// Add a message to recent history
    pub async fn add_message(&self, message: RoomMessage) {
        let mut messages = self.recent_messages.write().await;
        let mut last_msg = self.last_message_at.write().await;
        let mut count = self.message_count.write().await;

        // Add message
        messages.push(message);

        // Trim if over limit
        if messages.len() > self.max_recent_messages {
            messages.remove(0);
        }

        // Update stats
        *last_msg = current_timestamp();
        *count += 1;
    }

    /// Get recent messages
    pub async fn get_recent_messages(&self, limit: Option<usize>) -> Vec<RoomMessage> {
        let messages = self.recent_messages.read().await;
        let limit = limit.unwrap_or(self.max_recent_messages);
        messages.iter().rev().take(limit).cloned().collect()
    }

    /// Get message count
    pub async fn get_message_count(&self) -> u64 {
        *self.message_count.read().await
    }

    /// Set room metadata
    pub async fn set_metadata(&self, key: String, value: String) {
        let mut metadata = self.metadata.write().await;
        metadata.insert(key, value);
    }

    /// Get room metadata
    pub async fn get_metadata(&self, key: &str) -> Option<String> {
        let metadata = self.metadata.read().await;
        metadata.get(key).cloned()
    }

    /// Check if user can perform action based on role
    pub async fn user_can_moderate(&self, user_id: UserId) -> bool {
        let members = self.members.read().await;
        members
            .get(&user_id)
            .map(|m| m.role.can_moderate())
            .unwrap_or(false)
    }

    /// Check if user can manage members
    pub async fn user_can_manage_members(&self, user_id: UserId) -> bool {
        let members = self.members.read().await;
        members
            .get(&user_id)
            .map(|m| m.role.can_manage_members())
            .unwrap_or(false)
    }
}

/// Room manager for tracking all rooms and user memberships
pub struct RoomManager {
    /// All rooms indexed by room ID
    rooms: RwLock<HashMap<RoomId, Arc<Room>>>,
    /// User to rooms mapping (for fast lookup of user's rooms)
    user_rooms: RwLock<HashMap<UserId, HashSet<RoomId>>>,
    /// Next room ID (for auto-increment)
    next_room_id: RwLock<RoomId>,
    /// Created timestamp
    created_at: Instant,
}

impl RoomManager {
    /// Create a new room manager
    pub fn new() -> Self {
        Self {
            rooms: RwLock::new(HashMap::new()),
            user_rooms: RwLock::new(HashMap::new()),
            next_room_id: RwLock::new(1),
            created_at: Instant::now(),
        }
    }

    /// Create a new room
    pub async fn create_room(
        &self,
        name: String,
        room_type: RoomType,
        creator: RoomMember,
    ) -> Arc<Room> {
        let room_id = {
            let mut next_id = self.next_room_id.write().await;
            let id = *next_id;
            *next_id += 1;
            id
        };

        let room = Arc::new(Room::new(room_id, name, room_type));

        // Add creator as owner
        let creator_id = creator.user_id;
        room.add_member(creator.with_role(MemberRole::Owner)).await;

        // Register room
        {
            let mut rooms = self.rooms.write().await;
            rooms.insert(room_id, Arc::clone(&room));
        }

        // Update user's room list
        {
            let mut user_rooms = self.user_rooms.write().await;
            user_rooms
                .entry(creator_id)
                .or_insert_with(HashSet::new)
                .insert(room_id);
        }

        room
    }

    /// Create a room with a specific ID (for pre-defined rooms)
    pub async fn create_room_with_id(
        &self,
        room_id: RoomId,
        name: String,
        room_type: RoomType,
    ) -> Arc<Room> {
        let room = Arc::new(Room::new(room_id, name, room_type));

        {
            let mut rooms = self.rooms.write().await;
            rooms.insert(room_id, Arc::clone(&room));
        }

        // Update next_room_id if necessary
        {
            let mut next_id = self.next_room_id.write().await;
            if room_id >= *next_id {
                *next_id = room_id + 1;
            }
        }

        room
    }

    /// Get a room by ID
    pub async fn get_room(&self, room_id: RoomId) -> Option<Arc<Room>> {
        let rooms = self.rooms.read().await;
        rooms.get(&room_id).cloned()
    }

    /// Delete a room
    pub async fn delete_room(&self, room_id: RoomId) -> Option<Arc<Room>> {
        let room = {
            let mut rooms = self.rooms.write().await;
            rooms.remove(&room_id)
        };

        if let Some(ref room) = room {
            // Remove room from all users' room lists
            let member_ids = room.get_member_ids().await;
            let mut user_rooms = self.user_rooms.write().await;
            for user_id in member_ids {
                if let Some(rooms) = user_rooms.get_mut(&user_id) {
                    rooms.remove(&room_id);
                }
            }
        }

        room
    }

    /// Add a user to a room
    pub async fn join_room(&self, room_id: RoomId, member: RoomMember) -> bool {
        let user_id = member.user_id;

        let room = {
            let rooms = self.rooms.read().await;
            rooms.get(&room_id).cloned()
        };

        if let Some(room) = room {
            if room.add_member(member).await {
                let mut user_rooms = self.user_rooms.write().await;
                user_rooms
                    .entry(user_id)
                    .or_insert_with(HashSet::new)
                    .insert(room_id);
                return true;
            }
        }

        false
    }

    /// Remove a user from a room
    pub async fn leave_room(&self, room_id: RoomId, user_id: UserId) -> bool {
        let room = {
            let rooms = self.rooms.read().await;
            rooms.get(&room_id).cloned()
        };

        if let Some(room) = room {
            if room.remove_member(user_id).await.is_some() {
                let mut user_rooms = self.user_rooms.write().await;
                if let Some(rooms) = user_rooms.get_mut(&user_id) {
                    rooms.remove(&room_id);
                }
                return true;
            }
        }

        false
    }

    /// Get all rooms a user is a member of
    pub async fn get_user_rooms(&self, user_id: UserId) -> Vec<Arc<Room>> {
        let room_ids = {
            let user_rooms = self.user_rooms.read().await;
            user_rooms
                .get(&user_id)
                .map(|ids| ids.iter().copied().collect::<Vec<_>>())
                .unwrap_or_default()
        };

        let rooms = self.rooms.read().await;
        room_ids
            .into_iter()
            .filter_map(|id| rooms.get(&id).cloned())
            .collect()
    }

    /// Get room IDs for a user
    pub async fn get_user_room_ids(&self, user_id: UserId) -> Vec<RoomId> {
        let user_rooms = self.user_rooms.read().await;
        user_rooms
            .get(&user_id)
            .map(|ids| ids.iter().copied().collect())
            .unwrap_or_default()
    }

    /// Check if a user is in a room
    pub async fn is_user_in_room(&self, user_id: UserId, room_id: RoomId) -> bool {
        let user_rooms = self.user_rooms.read().await;
        user_rooms
            .get(&user_id)
            .map(|rooms| rooms.contains(&room_id))
            .unwrap_or(false)
    }

    /// Get all room IDs
    pub async fn get_all_room_ids(&self) -> Vec<RoomId> {
        let rooms = self.rooms.read().await;
        rooms.keys().copied().collect()
    }

    /// Get room count
    pub async fn room_count(&self) -> usize {
        let rooms = self.rooms.read().await;
        rooms.len()
    }

    /// Get total user count across all rooms (counting unique users)
    pub async fn total_user_count(&self) -> usize {
        let user_rooms = self.user_rooms.read().await;
        user_rooms.len()
    }

    /// Remove user from all rooms (e.g., on disconnect)
    pub async fn remove_user_from_all_rooms(&self, user_id: UserId) -> Vec<RoomId> {
        let room_ids = {
            let mut user_rooms = self.user_rooms.write().await;
            user_rooms
                .remove(&user_id)
                .map(|ids| ids.into_iter().collect::<Vec<_>>())
                .unwrap_or_default()
        };

        let rooms = self.rooms.read().await;
        for room_id in &room_ids {
            if let Some(room) = rooms.get(room_id) {
                room.remove_member(user_id).await;
            }
        }

        room_ids
    }

    /// Find or create a direct message room between two users
    pub async fn get_or_create_dm_room(
        &self,
        user1_id: UserId,
        user1_name: String,
        user2_id: UserId,
        user2_name: String,
    ) -> Arc<Room> {
        // Check if DM room already exists between these users
        let user1_rooms = self.get_user_room_ids(user1_id).await;
        let rooms = self.rooms.read().await;

        for room_id in user1_rooms {
            if let Some(room) = rooms.get(&room_id) {
                if room.room_type == RoomType::Direct {
                    if room.is_member(user2_id).await {
                        return Arc::clone(room);
                    }
                }
            }
        }
        drop(rooms);

        // Create new DM room
        let room_name = format!("DM: {} & {}", user1_name, user2_name);
        let member1 = RoomMember::new(user1_id, user1_name);
        let room = self.create_room(room_name, RoomType::Direct, member1).await;

        // Add second user
        let member2 = RoomMember::new(user2_id, user2_name);
        self.join_room(room.id, member2).await;

        room
    }

    /// Get uptime of the room manager
    pub fn uptime(&self) -> std::time::Duration {
        self.created_at.elapsed()
    }
}

impl Default for RoomManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_room_creation() {
        let manager = RoomManager::new();

        let creator = RoomMember::new(1, "alice".to_string());
        let room = manager
            .create_room("Test Room".to_string(), RoomType::Group, creator)
            .await;

        assert_eq!(room.name, "Test Room");
        assert_eq!(room.room_type, RoomType::Group);
        assert!(room.is_member(1).await);
    }

    #[tokio::test]
    async fn test_join_leave_room() {
        let manager = RoomManager::new();

        let creator = RoomMember::new(1, "alice".to_string());
        let room = manager
            .create_room("Test Room".to_string(), RoomType::Group, creator)
            .await;

        // Join
        let bob = RoomMember::new(2, "bob".to_string());
        assert!(manager.join_room(room.id, bob).await);
        assert!(room.is_member(2).await);
        assert_eq!(room.member_count().await, 2);

        // Leave
        assert!(manager.leave_room(room.id, 2).await);
        assert!(!room.is_member(2).await);
        assert_eq!(room.member_count().await, 1);
    }

    #[tokio::test]
    async fn test_user_rooms() {
        let manager = RoomManager::new();

        let creator = RoomMember::new(1, "alice".to_string());
        let room1 = manager
            .create_room("Room 1".to_string(), RoomType::Group, creator.clone())
            .await;

        let creator2 = RoomMember::new(1, "alice".to_string());
        let room2 = manager
            .create_room("Room 2".to_string(), RoomType::Group, creator2)
            .await;

        let user_rooms = manager.get_user_rooms(1).await;
        assert_eq!(user_rooms.len(), 2);

        let room_ids = manager.get_user_room_ids(1).await;
        assert!(room_ids.contains(&room1.id));
        assert!(room_ids.contains(&room2.id));
    }

    #[tokio::test]
    async fn test_room_messages() {
        let room = Room::new(1, "Test".to_string(), RoomType::Group);

        let msg = RoomMessage {
            message_id: 1,
            room_id: 1,
            sender: UserInfo {
                user_id: 1,
                username: "alice".to_string(),
                avatar_url: None,
            },
            content: "Hello".to_string(),
            timestamp: current_timestamp(),
            reply_to: None,
            attachments: vec![],
            nonce: Some("abc".to_string()),
        };

        room.add_message(msg.clone()).await;
        assert_eq!(room.get_message_count().await, 1);

        let recent = room.get_recent_messages(Some(10)).await;
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].content, "Hello");
    }

    #[tokio::test]
    async fn test_member_roles() {
        let room = Room::new(1, "Test".to_string(), RoomType::Group);

        let owner = RoomMember::new(1, "owner".to_string()).with_role(MemberRole::Owner);
        let member = RoomMember::new(2, "member".to_string());

        room.add_member(owner).await;
        room.add_member(member).await;

        assert!(room.user_can_manage_members(1).await);
        assert!(!room.user_can_manage_members(2).await);

        // Promote member to admin
        room.set_member_role(2, MemberRole::Admin).await;
        assert!(room.user_can_manage_members(2).await);
    }

    #[tokio::test]
    async fn test_dm_room() {
        let manager = RoomManager::new();

        let room1 = manager
            .get_or_create_dm_room(1, "alice".to_string(), 2, "bob".to_string())
            .await;

        // Getting DM again should return same room
        let room2 = manager
            .get_or_create_dm_room(1, "alice".to_string(), 2, "bob".to_string())
            .await;

        assert_eq!(room1.id, room2.id);
        assert_eq!(room1.room_type, RoomType::Direct);
    }

    #[tokio::test]
    async fn test_remove_user_from_all_rooms() {
        let manager = RoomManager::new();

        let alice = RoomMember::new(1, "alice".to_string());
        let room1 = manager
            .create_room("Room 1".to_string(), RoomType::Group, alice.clone())
            .await;

        let bob = RoomMember::new(2, "bob".to_string());
        manager.join_room(room1.id, bob.clone()).await;

        let alice2 = RoomMember::new(1, "alice".to_string());
        let room2 = manager
            .create_room("Room 2".to_string(), RoomType::Group, alice2)
            .await;
        manager
            .join_room(room2.id, RoomMember::new(2, "bob".to_string()))
            .await;

        // Remove bob from all rooms
        let removed_from = manager.remove_user_from_all_rooms(2).await;
        assert_eq!(removed_from.len(), 2);

        assert!(!room1.is_member(2).await);
        assert!(!room2.is_member(2).await);
    }
}
