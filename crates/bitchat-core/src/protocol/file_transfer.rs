//! File transfer protocol for BitChat
//!
//! This module implements the file transfer protocol allowing users to send files
//! securely through the BitChat network using chunked, encrypted transfer.

use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::protocol::message::NoisePayloadType;
use crate::types::{PeerId, Timestamp};
use crate::{BitchatError, Result};

// ----------------------------------------------------------------------------
// Constants
// ----------------------------------------------------------------------------

/// Maximum size of a single file chunk (16KB)
pub const MAX_CHUNK_SIZE: usize = 16 * 1024;

/// Maximum file size supported (100MB)
pub const MAX_FILE_SIZE: usize = 100 * 1024 * 1024;

/// File transfer timeout in seconds (30 minutes)
pub const TRANSFER_TIMEOUT_SECONDS: u64 = 30 * 60;

// ----------------------------------------------------------------------------
// Core Types
// ----------------------------------------------------------------------------

/// Unique identifier for a file transfer session
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FileTransferId(String);

impl FileTransferId {
    /// Generate a new random file transfer ID
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

impl core::fmt::Display for FileTransferId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// File metadata and information
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileMetadata {
    /// Original filename
    pub filename: String,
    /// File size in bytes
    pub size: u64,
    /// MIME type (optional)
    pub mime_type: Option<String>,
    /// SHA-256 hash of the complete file
    pub hash: FileHash,
    /// File creation timestamp
    pub created_at: Timestamp,
}

impl FileMetadata {
    /// Create new file metadata
    pub fn new(filename: String, size: u64, mime_type: Option<String>, data: &[u8]) -> Self {
        let hash = FileHash::from_data(data);
        Self {
            filename,
            size,
            mime_type,
            hash,
            created_at: Timestamp::now(),
        }
    }

    /// Validate file metadata
    pub fn validate(&self) -> Result<()> {
        if self.filename.is_empty() {
            return Err(BitchatError::invalid_packet("Filename cannot be empty"));
        }

        if self.size == 0 {
            return Err(BitchatError::invalid_packet("File size cannot be zero"));
        }

        if self.size > MAX_FILE_SIZE as u64 {
            return Err(BitchatError::invalid_packet(
                "File size exceeds maximum allowed",
            ));
        }

        Ok(())
    }
}

/// SHA-256 hash of file content
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileHash([u8; 32]);

impl FileHash {
    /// Create hash from raw bytes
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Calculate hash from file data
    pub fn from_data(data: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(data);
        let result = hasher.finalize();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);
        Self(hash)
    }

    /// Get raw bytes
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Verify data matches this hash
    pub fn verify(&self, data: &[u8]) -> bool {
        let computed_hash = Self::from_data(data);
        computed_hash == *self
    }
}

impl core::fmt::Display for FileHash {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", hex::encode(self.0))
    }
}

// ----------------------------------------------------------------------------
// File Transfer Messages
// ----------------------------------------------------------------------------

/// File transfer offer message (initiates transfer)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileOffer {
    /// Unique transfer ID
    pub transfer_id: FileTransferId,
    /// File metadata
    pub metadata: FileMetadata,
    /// Optional description or message
    pub description: Option<String>,
    /// Transfer expiration timestamp
    pub expires_at: Timestamp,
}

impl FileOffer {
    /// Create a new file offer
    pub fn new(metadata: FileMetadata, description: Option<String>) -> Result<Self> {
        metadata.validate()?;

        let expires_at = Timestamp::now() + (TRANSFER_TIMEOUT_SECONDS * 1000); // Convert to milliseconds

        Ok(Self {
            transfer_id: FileTransferId::generate(),
            metadata,
            description,
            expires_at,
        })
    }

    /// Check if offer has expired
    pub fn is_expired(&self) -> bool {
        Timestamp::now() > self.expires_at
    }

    /// Get total number of chunks needed for this file
    pub fn total_chunks(&self) -> u32 {
        self.metadata.size.div_ceil(MAX_CHUNK_SIZE as u64) as u32
    }
}

/// File transfer acceptance message
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileAccept {
    /// Transfer ID being accepted
    pub transfer_id: FileTransferId,
    /// Whether the transfer is accepted or rejected
    pub accepted: bool,
    /// Optional reason for rejection
    pub reason: Option<String>,
}

impl FileAccept {
    /// Create acceptance message
    pub fn accept(transfer_id: FileTransferId) -> Self {
        Self {
            transfer_id,
            accepted: true,
            reason: None,
        }
    }

    /// Create rejection message
    pub fn reject(transfer_id: FileTransferId, reason: Option<String>) -> Self {
        Self {
            transfer_id,
            accepted: false,
            reason,
        }
    }
}

/// File transfer chunk message
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileChunk {
    /// Transfer ID this chunk belongs to
    pub transfer_id: FileTransferId,
    /// Sequential chunk number (0-based)
    pub chunk_index: u32,
    /// Total number of chunks in the file
    pub total_chunks: u32,
    /// Chunk data
    pub data: Vec<u8>,
    /// Hash of this specific chunk for verification
    pub chunk_hash: FileHash,
}

impl FileChunk {
    /// Create a new file chunk
    pub fn new(
        transfer_id: FileTransferId,
        chunk_index: u32,
        total_chunks: u32,
        data: Vec<u8>,
    ) -> Result<Self> {
        if data.is_empty() {
            return Err(BitchatError::invalid_packet("Chunk data cannot be empty"));
        }

        if data.len() > MAX_CHUNK_SIZE {
            return Err(BitchatError::invalid_packet("Chunk size exceeds maximum"));
        }

        if chunk_index >= total_chunks {
            return Err(BitchatError::invalid_packet("Chunk index out of range"));
        }

        let chunk_hash = FileHash::from_data(&data);

        Ok(Self {
            transfer_id,
            chunk_index,
            total_chunks,
            data,
            chunk_hash,
        })
    }

    /// Verify chunk data integrity
    pub fn verify(&self) -> bool {
        self.chunk_hash.verify(&self.data)
    }

    /// Get chunk size
    pub fn size(&self) -> usize {
        self.data.len()
    }
}

/// File transfer completion message
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileComplete {
    /// Transfer ID that completed
    pub transfer_id: FileTransferId,
    /// Whether transfer was successful
    pub success: bool,
    /// Optional error message if transfer failed
    pub error: Option<String>,
    /// Final hash of assembled file (for verification)
    pub final_hash: Option<FileHash>,
}

impl FileComplete {
    /// Create success completion message
    pub fn success(transfer_id: FileTransferId, final_hash: FileHash) -> Self {
        Self {
            transfer_id,
            success: true,
            error: None,
            final_hash: Some(final_hash),
        }
    }

    /// Create failure completion message
    pub fn failure(transfer_id: FileTransferId, error: String) -> Self {
        Self {
            transfer_id,
            success: false,
            error: Some(error),
            final_hash: None,
        }
    }
}

// ----------------------------------------------------------------------------
// File Transfer Protocol Messages
// ----------------------------------------------------------------------------

/// All file transfer message types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileTransferMessage {
    /// File transfer offer
    Offer(FileOffer),
    /// File transfer acceptance/rejection
    Accept(FileAccept),
    /// File chunk data
    Chunk(FileChunk),
    /// Transfer completion notification
    Complete(FileComplete),
}

impl FileTransferMessage {
    /// Get the transfer ID for this message
    pub fn transfer_id(&self) -> &FileTransferId {
        match self {
            FileTransferMessage::Offer(offer) => &offer.transfer_id,
            FileTransferMessage::Accept(accept) => &accept.transfer_id,
            FileTransferMessage::Chunk(chunk) => &chunk.transfer_id,
            FileTransferMessage::Complete(complete) => &complete.transfer_id,
        }
    }

    /// Get the corresponding NoisePayloadType for this message
    pub fn payload_type(&self) -> NoisePayloadType {
        match self {
            FileTransferMessage::Offer(_) => NoisePayloadType::FileOffer,
            FileTransferMessage::Accept(_) => NoisePayloadType::FileAccept,
            FileTransferMessage::Chunk(_) => NoisePayloadType::FileChunk,
            FileTransferMessage::Complete(_) => NoisePayloadType::FileComplete,
        }
    }
}

// ----------------------------------------------------------------------------
// File Transfer Session Management
// ----------------------------------------------------------------------------

/// Status of a file transfer session
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransferStatus {
    /// Transfer has been offered but not yet accepted
    Offered,
    /// Transfer has been accepted and is in progress
    InProgress,
    /// Transfer completed successfully
    Completed,
    /// Transfer failed or was rejected
    Failed,
    /// Transfer was cancelled by either party
    Cancelled,
    /// Transfer expired before completion
    Expired,
}

/// File transfer session state
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileTransferSession {
    /// Unique transfer ID
    pub transfer_id: FileTransferId,
    /// Peer ID of sender
    pub sender: PeerId,
    /// Peer ID of recipient
    pub recipient: PeerId,
    /// File metadata
    pub metadata: FileMetadata,
    /// Current transfer status
    pub status: TransferStatus,
    /// Chunks received/sent (bit vector)
    pub chunks_received: Vec<bool>,
    /// Total number of chunks
    pub total_chunks: u32,
    /// Transfer start timestamp
    pub started_at: Timestamp,
    /// Last activity timestamp
    pub last_activity: Timestamp,
    /// Error message if transfer failed
    pub error: Option<String>,
}

impl FileTransferSession {
    /// Create a new transfer session from an offer
    pub fn from_offer(offer: &FileOffer, sender: PeerId, recipient: PeerId) -> Self {
        let total_chunks = offer.total_chunks();
        let chunks_received = vec![false; total_chunks as usize];
        let now = Timestamp::now();

        Self {
            transfer_id: offer.transfer_id.clone(),
            sender,
            recipient,
            metadata: offer.metadata.clone(),
            status: TransferStatus::Offered,
            chunks_received,
            total_chunks,
            started_at: now,
            last_activity: now,
            error: None,
        }
    }

    /// Mark transfer as accepted
    pub fn accept(&mut self) {
        self.status = TransferStatus::InProgress;
        self.last_activity = Timestamp::now();
    }

    /// Mark transfer as rejected or failed
    pub fn fail(&mut self, error: String) {
        self.status = TransferStatus::Failed;
        self.error = Some(error);
        self.last_activity = Timestamp::now();
    }

    /// Process a received chunk
    pub fn receive_chunk(&mut self, chunk: &FileChunk) -> Result<()> {
        if chunk.transfer_id != self.transfer_id {
            return Err(BitchatError::invalid_packet("Transfer ID mismatch"));
        }

        if chunk.chunk_index >= self.total_chunks {
            return Err(BitchatError::invalid_packet("Chunk index out of range"));
        }

        if !chunk.verify() {
            return Err(BitchatError::invalid_packet("Chunk verification failed"));
        }

        // Mark chunk as received
        self.chunks_received[chunk.chunk_index as usize] = true;
        self.last_activity = Timestamp::now();

        // Check if transfer is complete
        if self.chunks_received.iter().all(|&received| received) {
            self.status = TransferStatus::Completed;
        }

        Ok(())
    }

    /// Get transfer progress as percentage (0.0 to 1.0)
    pub fn progress(&self) -> f64 {
        if self.total_chunks == 0 {
            return 1.0;
        }

        let received_count = self
            .chunks_received
            .iter()
            .filter(|&&received| received)
            .count();
        received_count as f64 / self.total_chunks as f64
    }

    /// Check if transfer has expired
    pub fn is_expired(&self) -> bool {
        let now = Timestamp::now();
        let elapsed_millis = now - self.last_activity;
        elapsed_millis > (TRANSFER_TIMEOUT_SECONDS * 1000)
    }

    /// Get missing chunk indices
    pub fn missing_chunks(&self) -> Vec<u32> {
        self.chunks_received
            .iter()
            .enumerate()
            .filter(|(_, &received)| !received)
            .map(|(index, _)| index as u32)
            .collect()
    }

    /// Mark transfer as completed
    pub fn complete(&mut self, _final_hash: &FileHash) -> Result<()> {
        if self.status != TransferStatus::InProgress {
            return Err(BitchatError::invalid_packet("Transfer not in progress"));
        }

        if !self.chunks_received.iter().all(|&received| received) {
            return Err(BitchatError::invalid_packet("Not all chunks received"));
        }

        // Note: In a real implementation, you would verify the final hash here
        // against the assembled file data

        self.status = TransferStatus::Completed;
        self.last_activity = Timestamp::now();
        Ok(())
    }
}

// ----------------------------------------------------------------------------
// File Transfer Manager
// ----------------------------------------------------------------------------

/// Manages multiple file transfer sessions
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileTransferManager {
    /// Active transfer sessions
    sessions: hashbrown::HashMap<FileTransferId, FileTransferSession>,
}

impl FileTransferManager {
    /// Create a new file transfer manager
    pub fn new() -> Self {
        Self {
            sessions: hashbrown::HashMap::new(),
        }
    }

    /// Start a new outgoing transfer
    pub fn start_transfer(
        &mut self,
        offer: &FileOffer,
        sender: PeerId,
        recipient: PeerId,
    ) -> Result<&FileTransferSession> {
        if self.sessions.contains_key(&offer.transfer_id) {
            return Err(BitchatError::invalid_packet("Transfer ID already exists"));
        }

        let session = FileTransferSession::from_offer(offer, sender, recipient);
        let transfer_id = session.transfer_id.clone();
        self.sessions.insert(transfer_id.clone(), session);

        Ok(self.sessions.get(&transfer_id).unwrap())
    }

    /// Process an incoming file offer
    pub fn receive_offer(
        &mut self,
        offer: &FileOffer,
        sender: PeerId,
        recipient: PeerId,
    ) -> Result<&FileTransferSession> {
        if offer.is_expired() {
            return Err(BitchatError::invalid_packet("File offer has expired"));
        }

        if self.sessions.contains_key(&offer.transfer_id) {
            return Err(BitchatError::invalid_packet("Transfer ID already exists"));
        }

        let session = FileTransferSession::from_offer(offer, sender, recipient);
        let transfer_id = session.transfer_id.clone();
        self.sessions.insert(transfer_id.clone(), session);

        Ok(self.sessions.get(&transfer_id).unwrap())
    }

    /// Process a file acceptance message
    pub fn process_accept(&mut self, accept: &FileAccept) -> Result<()> {
        let session = self
            .sessions
            .get_mut(&accept.transfer_id)
            .ok_or_else(|| BitchatError::invalid_packet("Transfer not found"))?;

        if accept.accepted {
            session.accept();
        } else {
            let reason = accept
                .reason
                .clone()
                .unwrap_or_else(|| "Rejected".to_string());
            session.fail(reason);
        }

        Ok(())
    }

    /// Process a received file chunk
    pub fn process_chunk(&mut self, chunk: &FileChunk) -> Result<()> {
        let session = self
            .sessions
            .get_mut(&chunk.transfer_id)
            .ok_or_else(|| BitchatError::invalid_packet("Transfer not found"))?;

        session.receive_chunk(chunk)
    }

    /// Process a transfer completion message
    pub fn process_complete(&mut self, complete: &FileComplete) -> Result<()> {
        let session = self
            .sessions
            .get_mut(&complete.transfer_id)
            .ok_or_else(|| BitchatError::invalid_packet("Transfer not found"))?;

        if complete.success {
            if let Some(final_hash) = &complete.final_hash {
                session.complete(final_hash)?;
            } else {
                session.fail("No final hash provided".to_string());
            }
        } else {
            let error = complete
                .error
                .clone()
                .unwrap_or_else(|| "Transfer failed".to_string());
            session.fail(error);
        }

        Ok(())
    }

    /// Get a transfer session by ID
    pub fn get_session(&self, transfer_id: &FileTransferId) -> Option<&FileTransferSession> {
        self.sessions.get(transfer_id)
    }

    /// Get a mutable transfer session by ID
    pub fn get_session_mut(
        &mut self,
        transfer_id: &FileTransferId,
    ) -> Option<&mut FileTransferSession> {
        self.sessions.get_mut(transfer_id)
    }

    /// Get all active transfer sessions
    pub fn active_sessions(&self) -> Vec<&FileTransferSession> {
        self.sessions.values().collect()
    }

    /// Clean up expired or completed transfers
    pub fn cleanup_sessions(&mut self) {
        self.sessions.retain(|_, session| {
            !session.is_expired()
                && session.status != TransferStatus::Completed
                && session.status != TransferStatus::Failed
                && session.status != TransferStatus::Cancelled
        });
    }

    /// Cancel a transfer
    pub fn cancel_transfer(&mut self, transfer_id: &FileTransferId) -> Result<()> {
        let session = self
            .sessions
            .get_mut(transfer_id)
            .ok_or_else(|| BitchatError::invalid_packet("Transfer not found"))?;

        session.status = TransferStatus::Cancelled;
        session.last_activity = Timestamp::now();

        Ok(())
    }
}

impl Default for FileTransferManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_hash() {
        let data = b"Hello, World!";
        let hash1 = FileHash::from_data(data);
        let hash2 = FileHash::from_data(data);
        assert_eq!(hash1, hash2);
        assert!(hash1.verify(data));
        assert!(!hash1.verify(b"Different data"));
    }

    #[test]
    fn test_file_metadata() {
        let data = b"Test file content";
        let metadata = FileMetadata::new(
            "test.txt".to_string(),
            data.len() as u64,
            Some("text/plain".to_string()),
            data,
        );

        assert_eq!(metadata.filename, "test.txt");
        assert_eq!(metadata.size, data.len() as u64);
        assert!(metadata.validate().is_ok());
        assert!(metadata.hash.verify(data));
    }

    #[test]
    fn test_file_offer() {
        let data = b"Test file for offer";
        let metadata =
            FileMetadata::new("offer_test.txt".to_string(), data.len() as u64, None, data);

        let offer = FileOffer::new(metadata, Some("Test description".to_string())).unwrap();
        assert!(!offer.is_expired());
        assert!(offer.total_chunks() > 0);
    }

    #[test]
    fn test_file_chunk() {
        let transfer_id = FileTransferId::generate();
        let data = vec![1, 2, 3, 4, 5];

        let chunk = FileChunk::new(transfer_id.clone(), 0, 1, data.clone()).unwrap();
        assert_eq!(chunk.transfer_id, transfer_id);
        assert_eq!(chunk.chunk_index, 0);
        assert_eq!(chunk.total_chunks, 1);
        assert_eq!(chunk.data, data);
        assert!(chunk.verify());
    }

    #[test]
    fn test_transfer_session() {
        let data = b"Session test data";
        let metadata = FileMetadata::new(
            "session_test.txt".to_string(),
            data.len() as u64,
            None,
            data,
        );

        let offer = FileOffer::new(metadata, None).unwrap();
        let sender = PeerId::new([1; 8]);
        let recipient = PeerId::new([2; 8]);

        let mut session = FileTransferSession::from_offer(&offer, sender, recipient);
        assert_eq!(session.status, TransferStatus::Offered);
        assert_eq!(session.progress(), 0.0);

        session.accept();
        assert_eq!(session.status, TransferStatus::InProgress);
    }

    #[test]
    fn test_transfer_manager() {
        let mut manager = FileTransferManager::new();

        let data = b"Manager test data";
        let metadata = FileMetadata::new(
            "manager_test.txt".to_string(),
            data.len() as u64,
            None,
            data,
        );

        let offer = FileOffer::new(metadata, None).unwrap();
        let sender = PeerId::new([1; 8]);
        let recipient = PeerId::new([2; 8]);

        let session = manager.start_transfer(&offer, sender, recipient).unwrap();
        assert_eq!(session.transfer_id, offer.transfer_id);

        let accept = FileAccept::accept(offer.transfer_id.clone());
        manager.process_accept(&accept).unwrap();

        let session = manager.get_session(&offer.transfer_id).unwrap();
        assert_eq!(session.status, TransferStatus::InProgress);
    }
}
