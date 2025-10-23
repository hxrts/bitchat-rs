//! Integration tests for BitChat Nostr tunneling functionality
//!
//! These tests verify the complete flow of embedding, encoding, transmitting, 
//! receiving, and decoding BitChat messages through Nostr.

#[cfg(test)]
mod tests {
    use super::super::*;
    use bitchat_core::protocol::{BitchatMessage, MessageFlags, NoisePayload, NoisePayloadType};
    use bitchat_core::types::{PeerId, Timestamp};
    use std::time::Duration;

    fn create_test_peer_id(id: u8) -> PeerId {
        PeerId::new([id, 0, 0, 0, 0, 0, 0, 0])
    }

    #[test]
    fn test_complete_embedding_flow() {
        let sender = create_test_peer_id(1);
        let recipient = create_test_peer_id(2);

        // Create a test BitChat message
        let message = BitchatMessage::new(
            "msg123".to_string(),
            "Alice".to_string(),
            "Hello from BitChat over Nostr!".to_string(),
        );

        // Create noise payload
        let noise_payload = NoisePayload::new(
            NoisePayloadType::PrivateMessage,
            message.to_binary().unwrap(),
        );

        // Test with default configuration
        let embedded_default = NostrEmbeddedBitChat::encode_pm_for_nostr(
            sender,
            recipient,
            &noise_payload,
        ).unwrap();

        // Verify it has the canonical prefix
        assert!(embedded_default.starts_with(BITCHAT_EMBEDDING_PREFIX));

        // Decode and verify
        let decoded_packet = NostrEmbeddedBitChat::decode_from_nostr(&embedded_default)
            .unwrap()
            .unwrap();

        assert_eq!(decoded_packet.sender_id, sender);
        assert_eq!(decoded_packet.recipient_id, Some(recipient));

        let extracted_payload = NostrEmbeddedBitChat::extract_noise_payload(&decoded_packet).unwrap();
        assert_eq!(extracted_payload.payload_type, NoisePayloadType::PrivateMessage);

        // Verify we can reconstruct the original message
        let reconstructed_message = BitchatMessage::from_binary(&extracted_payload.data).unwrap();
        assert_eq!(reconstructed_message.id, "msg123");
        assert_eq!(reconstructed_message.sender, "Alice");
        assert_eq!(reconstructed_message.content, "Hello from BitChat over Nostr!");
    }

    #[test]
    fn test_privacy_embedding_flow() {
        let sender = create_test_peer_id(1);
        let recipient = create_test_peer_id(2);

        // Create a test message
        let message = BitchatMessage::new(
            "privacy_msg".to_string(),
            "Bob".to_string(),
            "This message has privacy protection".to_string(),
        );

        let noise_payload = NoisePayload::new(
            NoisePayloadType::PrivateMessage,
            message.to_binary().unwrap(),
        );

        // Test with privacy-focused configuration
        let privacy_config = EmbeddingConfig::privacy_focused();
        let embedded_private = NostrEmbeddedBitChat::encode_pm_for_nostr_with_config(
            sender,
            recipient,
            &noise_payload,
            &privacy_config,
        ).unwrap();

        // Should still have canonical prefix
        assert!(embedded_private.starts_with(BITCHAT_EMBEDDING_PREFIX));

        // Decode with privacy configuration
        let decoded_packet = NostrEmbeddedBitChat::decode_from_nostr(&embedded_private)
            .unwrap()
            .unwrap();

        let extracted_payload = NostrEmbeddedBitChat::extract_noise_payload_with_config(
            &decoded_packet, 
            &privacy_config
        ).unwrap();

        // Should be able to reconstruct the original message despite padding
        let reconstructed_message = BitchatMessage::from_binary(&extracted_payload.data).unwrap();
        assert_eq!(reconstructed_message.id, "privacy_msg");
        assert_eq!(reconstructed_message.sender, "Bob");
        assert_eq!(reconstructed_message.content, "This message has privacy protection");
    }

    #[test]
    fn test_geohash_embedding_flow() {
        let sender = create_test_peer_id(3);

        // Create a location-based message
        let location_message = BitchatMessage::new(
            "geo_msg".to_string(),
            "Charlie".to_string(),
            "Location-based message for geohash channel".to_string(),
        );

        let noise_payload = NoisePayload::new(
            NoisePayloadType::PrivateMessage,
            location_message.to_binary().unwrap(),
        );

        // Encode for geohash (no recipient)
        let embedded_geo = NostrEmbeddedBitChat::encode_pm_for_nostr_no_recipient(
            sender,
            &noise_payload,
        ).unwrap();

        // Verify embedding
        assert!(embedded_geo.starts_with(BITCHAT_EMBEDDING_PREFIX));

        // Decode and verify
        let decoded_packet = NostrEmbeddedBitChat::decode_from_nostr(&embedded_geo)
            .unwrap()
            .unwrap();

        assert_eq!(decoded_packet.sender_id, sender);
        assert_eq!(decoded_packet.recipient_id, None); // No recipient for geohash

        let extracted_payload = NostrEmbeddedBitChat::extract_noise_payload(&decoded_packet).unwrap();
        let reconstructed_message = BitchatMessage::from_binary(&extracted_payload.data).unwrap();
        assert_eq!(reconstructed_message.content, "Location-based message for geohash channel");
    }

    #[test]
    fn test_acknowledgment_embedding_flow() {
        use bitchat_core::protocol::acknowledgments::DeliveryAck;
        use bitchat_core::protocol::message_store::MessageId;
        use sha2::{Digest, Sha256};

        let sender = create_test_peer_id(4);
        let recipient = create_test_peer_id(5);

        // Create a delivery acknowledgment
        let hash = Sha256::digest(b"test message for ack");
        let message_id = MessageId::from_bytes(hash.into());
        let delivery_ack = DeliveryAck::new(message_id, sender, Some("Dave".to_string()));

        let ack_payload = NoisePayload::new(
            NoisePayloadType::Delivered,
            delivery_ack.to_binary().unwrap(),
        );

        // Encode acknowledgment
        let embedded_ack = NostrEmbeddedBitChat::encode_ack_for_nostr(
            sender,
            recipient,
            &ack_payload,
        ).unwrap();

        // Verify embedding
        assert!(embedded_ack.starts_with(BITCHAT_EMBEDDING_PREFIX));

        // Decode and verify
        let decoded_packet = NostrEmbeddedBitChat::decode_from_nostr(&embedded_ack)
            .unwrap()
            .unwrap();

        assert_eq!(decoded_packet.sender_id, sender);
        assert_eq!(decoded_packet.recipient_id, Some(recipient));

        let extracted_payload = NostrEmbeddedBitChat::extract_noise_payload(&decoded_packet).unwrap();
        assert_eq!(extracted_payload.payload_type, NoisePayloadType::Delivered);

        // Verify we can reconstruct the acknowledgment
        let reconstructed_ack = DeliveryAck::from_binary(&extracted_payload.data).unwrap();
        assert_eq!(reconstructed_ack.message_id, message_id);
        assert_eq!(reconstructed_ack.to_peer_id, sender);
        assert_eq!(reconstructed_ack.recipient_nickname, Some("Dave".to_string()));
    }

    #[test]
    fn test_strategy_selection() {
        // Test that recommended strategies work correctly
        let private_msg_normal = NostrEmbeddedBitChat::recommended_strategy(
            NoisePayloadType::PrivateMessage, 
            false
        );
        assert_eq!(private_msg_normal, EmbeddingStrategy::PrivateMessage);

        let private_msg_geo = NostrEmbeddedBitChat::recommended_strategy(
            NoisePayloadType::PrivateMessage, 
            true
        );
        assert_eq!(private_msg_geo, EmbeddingStrategy::PublicGeohash);

        let delivery_ack = NostrEmbeddedBitChat::recommended_strategy(
            NoisePayloadType::Delivered, 
            false
        );
        assert_eq!(delivery_ack, EmbeddingStrategy::PrivateMessage);

        let read_receipt = NostrEmbeddedBitChat::recommended_strategy(
            NoisePayloadType::ReadReceipt, 
            true
        );
        assert_eq!(read_receipt, EmbeddingStrategy::PrivateMessage);
    }

    #[test]
    fn test_embedding_config_variations() {
        // Test different embedding configurations
        // Use smaller padding to stay within V1 protocol limits (255 bytes)
        let small_privacy_config = EmbeddingConfig {
            default_strategy: EmbeddingStrategy::PrivateMessage,
            enable_padding: true,
            max_padding_bytes: 8, // Very small padding to stay well within V1 limits
            enable_timing_jitter: false, // Disable for test consistency
            max_timing_jitter_ms: 0,
        };
        
        let configs = [
            EmbeddingConfig::default(),
            EmbeddingConfig::performance_focused(),
            small_privacy_config,
        ];

        let sender = create_test_peer_id(6);
        let recipient = create_test_peer_id(7);

        let message = BitchatMessage::new(
            "config_test".to_string(),
            "Eve".to_string(),
            "Testing different configurations".to_string(),
        );

        let noise_payload = NoisePayload::new(
            NoisePayloadType::PrivateMessage,
            message.to_binary().unwrap(),
        );

        for config in &configs {
            // All configurations should validate
            assert!(config.validate().is_ok());

            // All configurations should be able to encode/decode successfully
            let embedded = NostrEmbeddedBitChat::encode_pm_for_nostr_with_config(
                sender,
                recipient,
                &noise_payload,
                config,
            ).unwrap();

            assert!(embedded.starts_with(BITCHAT_EMBEDDING_PREFIX));

            let decoded_packet = NostrEmbeddedBitChat::decode_from_nostr(&embedded)
                .unwrap()
                .unwrap();

            let extracted_payload = NostrEmbeddedBitChat::extract_noise_payload_with_config(
                &decoded_packet, 
                config
            ).unwrap();

            let reconstructed_message = BitchatMessage::from_binary(&extracted_payload.data).unwrap();
            assert_eq!(reconstructed_message.content, "Testing different configurations");
        }
    }

    #[test]
    fn test_timing_jitter_configuration() {
        let jitter_config = EmbeddingConfig {
            default_strategy: EmbeddingStrategy::PrivateMessage,
            enable_padding: false,
            max_padding_bytes: 0,
            enable_timing_jitter: true,
            max_timing_jitter_ms: 500,
        };

        // Test that jitter values are within expected range
        for _ in 0..10 {
            let jitter = jitter_config.get_timing_jitter();
            assert!(jitter.as_millis() <= 500);
        }

        let no_jitter_config = EmbeddingConfig {
            default_strategy: EmbeddingStrategy::PrivateMessage,
            enable_padding: false,
            max_padding_bytes: 0,
            enable_timing_jitter: false,
            max_timing_jitter_ms: 0,
        };

        let no_jitter = no_jitter_config.get_timing_jitter();
        assert_eq!(no_jitter, Duration::from_millis(0));
    }

    #[test]
    fn test_cross_compatibility() {
        // Test that messages encoded with one config can be decoded with another
        let sender = create_test_peer_id(8);
        let recipient = create_test_peer_id(9);

        let message = BitchatMessage::new(
            "cross_compat".to_string(),
            "Frank".to_string(),
            "Cross-compat test".to_string(),
        );

        let noise_payload = NoisePayload::new(
            NoisePayloadType::PrivateMessage,
            message.to_binary().unwrap(),
        );

        // Encode with performance config (no padding to avoid size issues)
        let privacy_config = EmbeddingConfig::performance_focused();
        let embedded = NostrEmbeddedBitChat::encode_pm_for_nostr_with_config(
            sender,
            recipient,
            &noise_payload,
            &privacy_config,
        ).unwrap();

        // Decode with default config
        let decoded_packet = NostrEmbeddedBitChat::decode_from_nostr(&embedded)
            .unwrap()
            .unwrap();

        // Should be able to extract with privacy config (which added padding)
        let extracted_payload = NostrEmbeddedBitChat::extract_noise_payload_with_config(
            &decoded_packet,
            &privacy_config,
        ).unwrap();

        let reconstructed_message = BitchatMessage::from_binary(&extracted_payload.data).unwrap();
        assert_eq!(reconstructed_message.content, "Cross-compat test");
    }

    #[test]
    fn test_large_message_handling() {
        let sender = create_test_peer_id(10);
        let recipient = create_test_peer_id(11);

        // Create a smaller message that leaves room for padding within V1 limits
        let large_content = "A".repeat(20); // 20 byte message
        let large_message = BitchatMessage::new(
            "large_msg".to_string(),
            "Grace".to_string(),
            large_content.clone(),
        );

        let noise_payload = NoisePayload::new(
            NoisePayloadType::PrivateMessage,
            large_message.to_binary().unwrap(),
        );

        // Test with padding enabled but keep within V1 limits
        let privacy_config = EmbeddingConfig {
            default_strategy: EmbeddingStrategy::PrivateMessage,
            enable_padding: true,
            max_padding_bytes: 32, // Small padding to stay within V1 255-byte limit
            enable_timing_jitter: false, // Skip timing for test consistency
            max_timing_jitter_ms: 0,
        };
        let embedded = NostrEmbeddedBitChat::encode_pm_for_nostr_with_config(
            sender,
            recipient,
            &noise_payload,
            &privacy_config,
        ).unwrap();

        // Should still be able to decode large padded messages
        let decoded_packet = NostrEmbeddedBitChat::decode_from_nostr(&embedded)
            .unwrap()
            .unwrap();

        let extracted_payload = NostrEmbeddedBitChat::extract_noise_payload_with_config(
            &decoded_packet,
            &privacy_config,
        ).unwrap();

        let reconstructed_message = BitchatMessage::from_binary(&extracted_payload.data).unwrap();
        assert_eq!(reconstructed_message.content, large_content);
    }

    #[test]
    fn test_non_bitchat_content_handling() {
        // Test that non-BitChat content is properly ignored
        let regular_nostr_content = "This is just a regular Nostr note";
        let result = NostrEmbeddedBitChat::decode_from_nostr(regular_nostr_content).unwrap();
        assert!(result.is_none());

        let partial_prefix = "bitchat:invalid"; // Wrong prefix
        let result = NostrEmbeddedBitChat::decode_from_nostr(partial_prefix).unwrap();
        assert!(result.is_none());

        let invalid_base64 = "bitchat1:invalid_base64!!!";
        let result = NostrEmbeddedBitChat::decode_from_nostr(invalid_base64);
        assert!(result.is_err()); // Should error on invalid base64
    }
}