//! Example demonstrating capability negotiation for BitChat interoperability
//!
//! This example shows how our enhanced Rust implementation can gracefully
//! interoperate with the canonical Swift implementation by detecting capabilities
//! and only using features that both peers support.

#[cfg(feature = "experimental")]
use bitchat_core::{
    CapabilityId, CapabilityManager, VersionHello, ImplementationInfo, 
    Capability, ProtocolVersion, PeerId, NegotiationStatus,
};

#[cfg(not(feature = "experimental"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Capability negotiation example requires 'experimental' feature flag");
    println!("Run with: cargo run --example capability_negotiation --features experimental");
    Ok(())
}

#[cfg(feature = "experimental")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("BitChat Capability Negotiation Example");
    println!("======================================\n");

    // Our Rust implementation peer
    let rust_peer = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
    let mut rust_manager = CapabilityManager::new(rust_peer)?;
    
    // Canonical Swift implementation peer (limited capabilities)
    let swift_peer = PeerId::new([9, 10, 11, 12, 13, 14, 15, 16]);
    
    println!("[Rust] Rust Implementation Capabilities:");
    let rust_hello = rust_manager.create_hello()?;
    for cap in &rust_hello.capabilities {
        println!("  [OK] {}", cap.id);
    }
    
    println!("\n[Swift] Simulated Swift Implementation Capabilities:");
    // Simulate canonical Swift implementation - only has core features
    let swift_hello = VersionHello::new(
        swift_peer,
        vec![ProtocolVersion::current()],
        vec![
            Capability::new(CapabilityId::core_messaging(), "1.0".to_string()),
            Capability::new(CapabilityId::noise_protocol(), "1.0".to_string()),
            Capability::new(CapabilityId::fragmentation(), "1.0".to_string()),
            Capability::new(CapabilityId::location_channels(), "1.0".to_string()),
            Capability::new(CapabilityId::mesh_sync(), "1.0".to_string()),
            Capability::new(CapabilityId::ble_transport(), "1.0".to_string()),
            Capability::new(CapabilityId::nostr_transport(), "1.0".to_string()),
        ],
        ImplementationInfo::new("bitchat-swift".to_string(), "1.0.0".to_string())
            .with_platform("iOS-arm64".to_string()),
    )?;
    
    for cap in &swift_hello.capabilities {
        println!("  [OK] {}", cap.id);
    }
    
    println!("\n[NEGOTIATION] Capability Negotiation:");
    let ack = rust_manager.process_hello(&swift_hello)?;
    
    println!("[MUTUAL] Negotiated Common Capabilities:");
    for cap in &ack.mutual_capabilities {
        println!("  [OK] {}", cap.id);
    }
    
    println!("\n[FEATURES] Feature Availability Check:");
    
    // Check core features (should work)
    println!("Core messaging: {}", 
        if rust_manager.should_use_feature(&swift_peer, &CapabilityId::core_messaging()) { 
            "[OK] Available" 
        } else { 
            "[ERROR] Not available" 
        }
    );
    
    println!("Location channels: {}", 
        if rust_manager.should_use_feature(&swift_peer, &CapabilityId::location_channels()) { 
            "[OK] Available" 
        } else { 
            "[ERROR] Not available" 
        }
    );
    
    // Check advanced features (should gracefully degrade)
    println!("File transfer: {}", 
        if rust_manager.should_use_feature(&swift_peer, &CapabilityId::file_transfer()) { 
            "[OK] Available" 
        } else { 
            "[ERROR] Not available - graceful degradation" 
        }
    );
    
    println!("Group messaging: {}", 
        if rust_manager.should_use_feature(&swift_peer, &CapabilityId::group_messaging()) { 
            "[OK] Available" 
        } else { 
            "[ERROR] Not available - graceful degradation" 
        }
    );
    
    println!("Multi-device sync: {}", 
        if rust_manager.should_use_feature(&swift_peer, &CapabilityId::multi_device_sync()) { 
            "[OK] Available" 
        } else { 
            "[ERROR] Not available - graceful degradation" 
        }
    );
    
    println!("\n[LEGACY] Canonical Implementation Compatibility:");
    
    // Simulate canonical implementation that doesn't respond to VersionHello
    let canonical_peer = PeerId::new([25, 26, 27, 28, 29, 30, 31, 32]);
    rust_manager.track_hello_sent(canonical_peer);
    
    println!("Status after sending VersionHello: {:?}", 
        rust_manager.get_negotiation_status(&canonical_peer));
    
    // After timeout (simulated)
    rust_manager.mark_as_legacy_peer(canonical_peer);
    
    println!("Status after timeout (legacy detected): {:?}", 
        rust_manager.get_negotiation_status(&canonical_peer));
    
    println!("\n[GRACEFUL] Feature Availability with Canonical Implementation:");
    
    // Core features available
    println!("Core messaging: {}", 
        if rust_manager.should_use_feature(&canonical_peer, &CapabilityId::core_messaging()) { 
            "[OK] Available" 
        } else { 
            "[ERROR] Not available" 
        }
    );
    
    println!("Location channels: {}", 
        if rust_manager.should_use_feature(&canonical_peer, &CapabilityId::location_channels()) { 
            "[OK] Available" 
        } else { 
            "[ERROR] Not available" 
        }
    );
    
    // Advanced features gracefully disabled
    println!("File transfer: {}", 
        if rust_manager.should_use_feature(&canonical_peer, &CapabilityId::file_transfer()) { 
            "[OK] Available" 
        } else { 
            "[GRACEFUL] Disabled - canonical implementation doesn't support" 
        }
    );
    
    println!("Group messaging: {}", 
        if rust_manager.should_use_feature(&canonical_peer, &CapabilityId::group_messaging()) { 
            "[OK] Available" 
        } else { 
            "[GRACEFUL] Disabled - canonical implementation doesn't support" 
        }
    );
    
    println!("\n[STRATEGY] Interoperability Strategy:");
    println!("• Send VersionHello to all new peers");
    println!("• If no response within 30 seconds, mark as legacy peer");
    println!("• Legacy peers get core capabilities only (what canonical supports)");
    println!("• Enhanced peers get full feature set after successful negotiation");
    println!("• Graceful degradation ensures compatibility across all BitChat clients");
    println!("• This implementation is MORE advanced than canonical - we lead in features");
    
    Ok(())
}