//! Group messaging protocol for BitChat
//!
//! This module implements group messaging functionality, allowing users to create
//! and participate in group conversations with multiple participants.

use alloc::{collections::BTreeMap, string::String, vec::Vec};
use serde::{Deserialize, Serialize};

use crate::protocol::message::NoisePayloadType;
use crate::types::{Fingerprint, PeerId, Timestamp};
use crate::{BitchatError, Result};

// ----------------------------------------------------------------------------
// Constants
// ----------------------------------------------------------------------------

/// Maximum number of members in a group
pub const MAX_GROUP_SIZE: usize = 256;

/// Maximum length of group name
pub const MAX_GROUP_NAME_LENGTH: usize = 128;

/// Maximum length of group description
pub const MAX_GROUP_DESCRIPTION_LENGTH: usize = 512;

/// Maximum length of member nickname
pub const MAX_MEMBER_NICKNAME_LENGTH: usize = 64;

// ----------------------------------------------------------------------------
// Core Types
// ----------------------------------------------------------------------------

/// Unique identifier for a group
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct GroupId(String);

impl GroupId {
    /// Generate a new random group ID
    pub fn generate() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    /// Create from string
    pub fn from_string(id: String) -> Self {
        Self(id)
    }

    /// Get the string representation
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl core::fmt::Display for GroupId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Group member with their role and metadata
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupMember {
    /// Member's peer ID
    pub peer_id: PeerId,
    /// Member's display nickname in the group
    pub nickname: String,
    /// Member's role in the group
    pub role: GroupRole,
    /// When the member joined the group
    pub joined_at: Timestamp,
    /// Member's public key fingerprint for verification
    pub fingerprint: Fingerprint,
}

impl GroupMember {
    /// Create a new group member
    pub fn new(
        peer_id: PeerId,
        nickname: String,
        fingerprint: Fingerprint,
        role: GroupRole,
    ) -> Result<Self> {
        if nickname.is_empty() {
            return Err(BitchatError::invalid_packet(
                "Member nickname cannot be empty",
            ));
        }

        if nickname.len() > MAX_MEMBER_NICKNAME_LENGTH {
            return Err(BitchatError::invalid_packet("Member nickname too long"));
        }

        Ok(Self {
            peer_id,
            nickname,
            role,
            joined_at: Timestamp::now(),
            fingerprint,
        })
    }
}

/// Member roles in a group
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GroupRole {
    /// Group creator and primary administrator
    Owner,
    /// Group administrator with most privileges
    Admin,
    /// Regular group member
    Member,
}

impl GroupRole {
    /// Check if this role can invite new members
    pub fn can_invite(&self) -> bool {
        matches!(self, GroupRole::Owner | GroupRole::Admin)
    }

    /// Check if this role can kick other members
    pub fn can_kick(&self) -> bool {
        matches!(self, GroupRole::Owner | GroupRole::Admin)
    }

    /// Check if this role can modify group metadata
    pub fn can_modify_group(&self) -> bool {
        matches!(self, GroupRole::Owner | GroupRole::Admin)
    }

    /// Check if this role can promote other members
    pub fn can_promote(&self) -> bool {
        matches!(self, GroupRole::Owner)
    }
}

/// Group metadata and configuration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupMetadata {
    /// Group unique identifier
    pub group_id: GroupId,
    /// Group display name
    pub name: String,
    /// Optional group description
    pub description: Option<String>,
    /// Group creation timestamp
    pub created_at: Timestamp,
    /// Group creator's peer ID
    pub creator: PeerId,
    /// Current group members (peer_id -> member)
    pub members: BTreeMap<PeerId, GroupMember>,
    /// Group avatar hash (optional)
    pub avatar_hash: Option<String>,
    /// Group settings
    pub settings: GroupSettings,
}

impl GroupMetadata {
    /// Create new group metadata
    pub fn new(
        name: String,
        description: Option<String>,
        creator: PeerId,
        creator_nickname: String,
        creator_fingerprint: Fingerprint,
    ) -> Result<Self> {
        if name.is_empty() {
            return Err(BitchatError::invalid_packet("Group name cannot be empty"));
        }

        if name.len() > MAX_GROUP_NAME_LENGTH {
            return Err(BitchatError::invalid_packet("Group name too long"));
        }

        if let Some(ref desc) = description {
            if desc.len() > MAX_GROUP_DESCRIPTION_LENGTH {
                return Err(BitchatError::invalid_packet("Group description too long"));
            }
        }

        let group_id = GroupId::generate();
        let mut members = BTreeMap::new();

        // Add creator as owner
        let creator_member = GroupMember::new(
            creator,
            creator_nickname,
            creator_fingerprint,
            GroupRole::Owner,
        )?;
        members.insert(creator, creator_member);

        Ok(Self {
            group_id,
            name,
            description,
            created_at: Timestamp::now(),
            creator,
            members,
            avatar_hash: None,
            settings: GroupSettings::default(),
        })
    }

    /// Add a member to the group
    pub fn add_member(&mut self, member: GroupMember) -> Result<()> {
        if self.members.len() >= MAX_GROUP_SIZE {
            return Err(BitchatError::invalid_packet("Group is at maximum capacity"));
        }

        if self.members.contains_key(&member.peer_id) {
            return Err(BitchatError::invalid_packet(
                "Member already exists in group",
            ));
        }

        self.members.insert(member.peer_id, member);
        Ok(())
    }

    /// Remove a member from the group
    pub fn remove_member(&mut self, peer_id: &PeerId) -> Result<GroupMember> {
        if *peer_id == self.creator {
            return Err(BitchatError::invalid_packet("Cannot remove group creator"));
        }

        self.members
            .remove(peer_id)
            .ok_or_else(|| BitchatError::invalid_packet("Member not found in group"))
    }

    /// Get a member by peer ID
    pub fn get_member(&self, peer_id: &PeerId) -> Option<&GroupMember> {
        self.members.get(peer_id)
    }

    /// Get a mutable member by peer ID
    pub fn get_member_mut(&mut self, peer_id: &PeerId) -> Option<&mut GroupMember> {
        self.members.get_mut(peer_id)
    }

    /// Check if a peer is a member of the group
    pub fn is_member(&self, peer_id: &PeerId) -> bool {
        self.members.contains_key(peer_id)
    }

    /// Get all member peer IDs
    pub fn member_ids(&self) -> Vec<PeerId> {
        self.members.keys().cloned().collect()
    }

    /// Get member count
    pub fn member_count(&self) -> usize {
        self.members.len()
    }

    /// Update group metadata (name, description, etc.)
    pub fn update(
        &mut self,
        name: Option<String>,
        description: Option<Option<String>>,
    ) -> Result<()> {
        if let Some(new_name) = name {
            if new_name.is_empty() {
                return Err(BitchatError::invalid_packet("Group name cannot be empty"));
            }
            if new_name.len() > MAX_GROUP_NAME_LENGTH {
                return Err(BitchatError::invalid_packet("Group name too long"));
            }
            self.name = new_name;
        }

        if let Some(new_description) = description {
            if let Some(ref desc) = new_description {
                if desc.len() > MAX_GROUP_DESCRIPTION_LENGTH {
                    return Err(BitchatError::invalid_packet("Group description too long"));
                }
            }
            self.description = new_description;
        }

        Ok(())
    }
}

/// Group settings and configuration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupSettings {
    /// Whether only admins can send messages
    pub admin_only_messages: bool,
    /// Whether members can invite others
    pub members_can_invite: bool,
    /// Whether to enable read receipts
    pub read_receipts_enabled: bool,
    /// Maximum message history to keep
    pub max_message_history: u32,
}

impl Default for GroupSettings {
    fn default() -> Self {
        Self {
            admin_only_messages: false,
            members_can_invite: true,
            read_receipts_enabled: true,
            max_message_history: 1000,
        }
    }
}

// ----------------------------------------------------------------------------
// Group Messages
// ----------------------------------------------------------------------------

/// Group creation message
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupCreate {
    /// Group metadata
    pub metadata: GroupMetadata,
    /// Initial invitation message
    pub invitation_message: Option<String>,
}

impl GroupCreate {
    /// Create a new group creation message
    pub fn new(metadata: GroupMetadata, invitation_message: Option<String>) -> Self {
        Self {
            metadata,
            invitation_message,
        }
    }

    /// Get the group ID
    pub fn group_id(&self) -> &GroupId {
        &self.metadata.group_id
    }
}

/// Group invitation message
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupInvite {
    /// Group ID being invited to
    pub group_id: GroupId,
    /// Current group metadata (for verification)
    pub group_metadata: GroupMetadata,
    /// Inviter's peer ID
    pub inviter: PeerId,
    /// Invitation message
    pub message: Option<String>,
    /// Invitation expiration timestamp
    pub expires_at: Timestamp,
}

impl GroupInvite {
    /// Create a new group invitation
    pub fn new(group_metadata: GroupMetadata, inviter: PeerId, message: Option<String>) -> Self {
        let expires_at = Timestamp::now() + (24 * 60 * 60 * 1000); // 24 hours

        Self {
            group_id: group_metadata.group_id.clone(),
            group_metadata,
            inviter,
            message,
            expires_at,
        }
    }

    /// Check if invitation has expired
    pub fn is_expired(&self) -> bool {
        Timestamp::now() > self.expires_at
    }
}

/// Group join message
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupJoin {
    /// Group ID being joined
    pub group_id: GroupId,
    /// Joiner's information
    pub member: GroupMember,
    /// Optional join message
    pub message: Option<String>,
}

impl GroupJoin {
    /// Create a new group join message
    pub fn new(group_id: GroupId, member: GroupMember, message: Option<String>) -> Self {
        Self {
            group_id,
            member,
            message,
        }
    }
}

/// Group leave message
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupLeave {
    /// Group ID being left
    pub group_id: GroupId,
    /// Leaver's peer ID
    pub peer_id: PeerId,
    /// Optional leave reason
    pub reason: Option<String>,
}

impl GroupLeave {
    /// Create a new group leave message
    pub fn new(group_id: GroupId, peer_id: PeerId, reason: Option<String>) -> Self {
        Self {
            group_id,
            peer_id,
            reason,
        }
    }
}

/// Group message content
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupMessage {
    /// Group ID this message belongs to
    pub group_id: GroupId,
    /// Message unique identifier
    pub message_id: String,
    /// Sender's peer ID
    pub sender: PeerId,
    /// Message content
    pub content: String,
    /// Message timestamp
    pub timestamp: Timestamp,
    /// Message reply reference (optional)
    pub reply_to: Option<String>,
    /// Mentioned members (optional)
    pub mentions: Option<Vec<PeerId>>,
}

impl GroupMessage {
    /// Create a new group message
    pub fn new(group_id: GroupId, sender: PeerId, content: String) -> Self {
        Self {
            group_id,
            message_id: uuid::Uuid::new_v4().to_string(),
            sender,
            content,
            timestamp: Timestamp::now(),
            reply_to: None,
            mentions: None,
        }
    }

    /// Create a reply message
    pub fn reply(group_id: GroupId, sender: PeerId, content: String, reply_to: String) -> Self {
        Self {
            group_id,
            message_id: uuid::Uuid::new_v4().to_string(),
            sender,
            content,
            timestamp: Timestamp::now(),
            reply_to: Some(reply_to),
            mentions: None,
        }
    }

    /// Add mentions to the message
    pub fn with_mentions(mut self, mentions: Vec<PeerId>) -> Self {
        self.mentions = Some(mentions);
        self
    }
}

/// Group metadata update message
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupUpdate {
    /// Group ID being updated
    pub group_id: GroupId,
    /// Updater's peer ID
    pub updater: PeerId,
    /// New group name (if changed)
    pub name: Option<String>,
    /// New group description (if changed)
    pub description: Option<Option<String>>,
    /// New group settings (if changed)
    pub settings: Option<GroupSettings>,
    /// Update timestamp
    pub timestamp: Timestamp,
}

impl GroupUpdate {
    /// Create a new group update message
    pub fn new(group_id: GroupId, updater: PeerId) -> Self {
        Self {
            group_id,
            updater,
            name: None,
            description: None,
            settings: None,
            timestamp: Timestamp::now(),
        }
    }

    /// Update group name
    pub fn with_name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }

    /// Update group description
    pub fn with_description(mut self, description: Option<String>) -> Self {
        self.description = Some(description);
        self
    }

    /// Update group settings
    pub fn with_settings(mut self, settings: GroupSettings) -> Self {
        self.settings = Some(settings);
        self
    }
}

/// Group member kick/remove message
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupKick {
    /// Group ID where kick occurred
    pub group_id: GroupId,
    /// Kicker's peer ID (must be admin/owner)
    pub kicker: PeerId,
    /// Kicked member's peer ID
    pub kicked_member: PeerId,
    /// Reason for kick
    pub reason: Option<String>,
    /// Kick timestamp
    pub timestamp: Timestamp,
}

impl GroupKick {
    /// Create a new group kick message
    pub fn new(
        group_id: GroupId,
        kicker: PeerId,
        kicked_member: PeerId,
        reason: Option<String>,
    ) -> Self {
        Self {
            group_id,
            kicker,
            kicked_member,
            reason,
            timestamp: Timestamp::now(),
        }
    }
}

// ----------------------------------------------------------------------------
// Group Message Wrapper
// ----------------------------------------------------------------------------

/// All group messaging message types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GroupMessagingMessage {
    /// Group creation
    Create(GroupCreate),
    /// Group invitation
    Invite(GroupInvite),
    /// Group join request/notification
    Join(GroupJoin),
    /// Group leave notification
    Leave(GroupLeave),
    /// Group chat message
    Message(GroupMessage),
    /// Group metadata update
    Update(GroupUpdate),
    /// Group member kick
    Kick(GroupKick),
}

impl GroupMessagingMessage {
    /// Get the group ID for this message
    pub fn group_id(&self) -> &GroupId {
        match self {
            GroupMessagingMessage::Create(create) => create.group_id(),
            GroupMessagingMessage::Invite(invite) => &invite.group_id,
            GroupMessagingMessage::Join(join) => &join.group_id,
            GroupMessagingMessage::Leave(leave) => &leave.group_id,
            GroupMessagingMessage::Message(message) => &message.group_id,
            GroupMessagingMessage::Update(update) => &update.group_id,
            GroupMessagingMessage::Kick(kick) => &kick.group_id,
        }
    }

    /// Get the corresponding NoisePayloadType for this message
    pub fn payload_type(&self) -> NoisePayloadType {
        match self {
            GroupMessagingMessage::Create(_) => NoisePayloadType::GroupCreate,
            GroupMessagingMessage::Invite(_) => NoisePayloadType::GroupInvite,
            GroupMessagingMessage::Join(_) => NoisePayloadType::GroupJoin,
            GroupMessagingMessage::Leave(_) => NoisePayloadType::GroupLeave,
            GroupMessagingMessage::Message(_) => NoisePayloadType::GroupMessage,
            GroupMessagingMessage::Update(_) => NoisePayloadType::GroupUpdate,
            GroupMessagingMessage::Kick(_) => NoisePayloadType::GroupKick,
        }
    }
}

// ----------------------------------------------------------------------------
// Group Manager
// ----------------------------------------------------------------------------

/// Manages group memberships and message processing
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupManager {
    /// Groups this peer is a member of (group_id -> metadata)
    groups: BTreeMap<GroupId, GroupMetadata>,
    /// Local peer ID
    local_peer_id: PeerId,
}

impl GroupManager {
    /// Create a new group manager
    pub fn new(local_peer_id: PeerId) -> Self {
        Self {
            groups: BTreeMap::new(),
            local_peer_id,
        }
    }

    /// Create a new group
    pub fn create_group(
        &mut self,
        name: String,
        description: Option<String>,
        nickname: String,
        fingerprint: Fingerprint,
    ) -> Result<GroupCreate> {
        let metadata =
            GroupMetadata::new(name, description, self.local_peer_id, nickname, fingerprint)?;

        let group_id = metadata.group_id.clone();
        self.groups.insert(group_id, metadata.clone());

        Ok(GroupCreate::new(metadata, None))
    }

    /// Process a group invitation
    pub fn process_invite(&mut self, invite: &GroupInvite) -> Result<()> {
        if invite.is_expired() {
            return Err(BitchatError::invalid_packet("Group invitation has expired"));
        }

        // Don't auto-join - this would typically trigger a user prompt
        // For now, just store the invitation for manual processing
        Ok(())
    }

    /// Join a group
    pub fn join_group(
        &mut self,
        group_metadata: GroupMetadata,
        nickname: String,
        fingerprint: Fingerprint,
    ) -> Result<GroupJoin> {
        let group_id = group_metadata.group_id.clone();

        if self.groups.contains_key(&group_id) {
            return Err(BitchatError::invalid_packet(
                "Already a member of this group",
            ));
        }

        let member =
            GroupMember::new(self.local_peer_id, nickname, fingerprint, GroupRole::Member)?;

        self.groups.insert(group_id.clone(), group_metadata);

        Ok(GroupJoin::new(group_id, member, None))
    }

    /// Leave a group
    pub fn leave_group(
        &mut self,
        group_id: &GroupId,
        reason: Option<String>,
    ) -> Result<GroupLeave> {
        if !self.groups.contains_key(group_id) {
            return Err(BitchatError::invalid_packet("Not a member of this group"));
        }

        self.groups.remove(group_id);

        Ok(GroupLeave::new(
            group_id.clone(),
            self.local_peer_id,
            reason,
        ))
    }

    /// Send a message to a group
    pub fn send_message(&self, group_id: &GroupId, content: String) -> Result<GroupMessage> {
        if !self.groups.contains_key(group_id) {
            return Err(BitchatError::invalid_packet("Not a member of this group"));
        }

        Ok(GroupMessage::new(
            group_id.clone(),
            self.local_peer_id,
            content,
        ))
    }

    /// Process an incoming group message
    pub fn process_message(&mut self, message: &GroupMessagingMessage) -> Result<()> {
        let group_id = message.group_id();

        match message {
            GroupMessagingMessage::Create(create) => {
                // Store the group metadata
                self.groups
                    .insert(group_id.clone(), create.metadata.clone());
            }

            GroupMessagingMessage::Join(join) => {
                if let Some(group) = self.groups.get_mut(group_id) {
                    group.add_member(join.member.clone())?;
                }
            }

            GroupMessagingMessage::Leave(leave) => {
                if let Some(group) = self.groups.get_mut(group_id) {
                    group.remove_member(&leave.peer_id)?;
                }
            }

            GroupMessagingMessage::Update(update) => {
                if let Some(group) = self.groups.get_mut(group_id) {
                    group.update(update.name.clone(), update.description.clone())?;

                    if let Some(ref settings) = update.settings {
                        group.settings = settings.clone();
                    }
                }
            }

            GroupMessagingMessage::Kick(kick) => {
                if let Some(group) = self.groups.get_mut(group_id) {
                    group.remove_member(&kick.kicked_member)?;
                }
            }

            GroupMessagingMessage::Invite(_) => {
                // Invitations are handled separately by process_invite
            }

            GroupMessagingMessage::Message(_) => {
                // Regular messages are just stored/displayed
                // Message storage would be handled by the message store
            }
        }

        Ok(())
    }

    /// Get group metadata by ID
    pub fn get_group(&self, group_id: &GroupId) -> Option<&GroupMetadata> {
        self.groups.get(group_id)
    }

    /// Get all groups this peer is a member of
    pub fn get_all_groups(&self) -> Vec<&GroupMetadata> {
        self.groups.values().collect()
    }

    /// Check if peer is a member of a group
    pub fn is_member_of(&self, group_id: &GroupId) -> bool {
        self.groups.contains_key(group_id)
    }

    /// Get groups where this peer has admin privileges
    pub fn get_admin_groups(&self) -> Vec<&GroupMetadata> {
        self.groups
            .values()
            .filter(|group| {
                group
                    .get_member(&self.local_peer_id)
                    .map(|member| member.role.can_kick())
                    .unwrap_or(false)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_member(id: u8, nickname: &str) -> GroupMember {
        let peer_id = PeerId::new([id; 8]);
        let fingerprint = Fingerprint::new([id; 32]);
        GroupMember::new(
            peer_id,
            nickname.to_string(),
            fingerprint,
            GroupRole::Member,
        )
        .unwrap()
    }

    #[test]
    fn test_group_creation() {
        let creator = PeerId::new([1; 8]);
        let fingerprint = Fingerprint::new([1; 32]);

        let metadata = GroupMetadata::new(
            "Test Group".to_string(),
            Some("A test group".to_string()),
            creator,
            "Creator".to_string(),
            fingerprint,
        )
        .unwrap();

        assert_eq!(metadata.name, "Test Group");
        assert_eq!(metadata.creator, creator);
        assert_eq!(metadata.member_count(), 1);
        assert!(metadata.is_member(&creator));
    }

    #[test]
    fn test_group_member_management() {
        let creator = PeerId::new([1; 8]);
        let fingerprint = Fingerprint::new([1; 32]);

        let mut metadata = GroupMetadata::new(
            "Test Group".to_string(),
            None,
            creator,
            "Creator".to_string(),
            fingerprint,
        )
        .unwrap();

        // Add a member
        let member = create_test_member(2, "Member");
        metadata.add_member(member.clone()).unwrap();

        assert_eq!(metadata.member_count(), 2);
        assert!(metadata.is_member(&member.peer_id));

        // Remove the member
        let removed = metadata.remove_member(&member.peer_id).unwrap();
        assert_eq!(removed.peer_id, member.peer_id);
        assert_eq!(metadata.member_count(), 1);
        assert!(!metadata.is_member(&member.peer_id));
    }

    #[test]
    fn test_group_manager() {
        let local_peer = PeerId::new([1; 8]);
        let fingerprint = Fingerprint::new([1; 32]);
        let mut manager = GroupManager::new(local_peer);

        // Create a group
        let group_create = manager
            .create_group(
                "Test Group".to_string(),
                Some("Description".to_string()),
                "Creator".to_string(),
                fingerprint,
            )
            .unwrap();

        assert_eq!(manager.get_all_groups().len(), 1);
        assert!(manager.is_member_of(group_create.group_id()));

        // Send a message
        let message = manager
            .send_message(group_create.group_id(), "Hello, group!".to_string())
            .unwrap();

        assert_eq!(message.sender, local_peer);
        assert_eq!(message.content, "Hello, group!");
    }

    #[test]
    fn test_group_roles() {
        assert!(GroupRole::Owner.can_invite());
        assert!(GroupRole::Owner.can_kick());
        assert!(GroupRole::Owner.can_modify_group());
        assert!(GroupRole::Owner.can_promote());

        assert!(GroupRole::Admin.can_invite());
        assert!(GroupRole::Admin.can_kick());
        assert!(GroupRole::Admin.can_modify_group());
        assert!(!GroupRole::Admin.can_promote());

        assert!(!GroupRole::Member.can_kick());
        assert!(!GroupRole::Member.can_modify_group());
        assert!(!GroupRole::Member.can_promote());
    }

    #[test]
    fn test_group_messages() {
        let group_id = GroupId::generate();
        let sender = PeerId::new([1; 8]);

        // Test regular message
        let message = GroupMessage::new(group_id.clone(), sender, "Hello!".to_string());

        assert_eq!(message.sender, sender);
        assert_eq!(message.content, "Hello!");
        assert!(message.reply_to.is_none());

        // Test reply message
        let reply = GroupMessage::reply(
            group_id,
            sender,
            "Reply!".to_string(),
            message.message_id.clone(),
        );

        assert_eq!(reply.reply_to, Some(message.message_id));
    }
}
