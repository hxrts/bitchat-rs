// TEMPORARY: Xcode 16+ linker workaround for C libraries
// This provides secp256k1 FFI bindings for iOS to bypass Xcode 16 partial linking bug
// DELETE THIS FILE when Xcode is fixed

use secp256k1::schnorr::Signature as SchnorrSignature;
use secp256k1::{ecdh, Keypair, Message, PublicKey, Secp256k1, SecretKey, XOnlyPublicKey};
use std::slice;

/// Generate a new random private key (32 bytes)
/// Returns 1 on success, 0 on failure
#[no_mangle]
pub extern "C" fn secp256k1_privkey_generate(out: *mut u8) -> i32 {
    let secp = Secp256k1::new();
    let (secret_key, _) = secp.generate_keypair(&mut secp256k1::rand::thread_rng());

    unsafe {
        let out_slice = slice::from_raw_parts_mut(out, 32);
        out_slice.copy_from_slice(&secret_key[..]);
    }
    1
}

/// Get public key from private key (33 bytes compressed)
/// Returns 1 on success, 0 on failure  
#[no_mangle]
pub extern "C" fn secp256k1_pubkey_from_privkey(privkey: *const u8, pubkey_out: *mut u8) -> i32 {
    let secp = Secp256k1::new();

    unsafe {
        let privkey_bytes = slice::from_raw_parts(privkey, 32);
        let secret_key = match SecretKey::from_slice(privkey_bytes) {
            Ok(sk) => sk,
            Err(_) => return 0,
        };

        let public_key = PublicKey::from_secret_key(&secp, &secret_key);
        let pubkey_slice = slice::from_raw_parts_mut(pubkey_out, 33);
        pubkey_slice.copy_from_slice(&public_key.serialize());
    }
    1
}

/// Get x-only public key (Schnorr, 32 bytes)
/// Returns 1 on success, 0 on failure
#[no_mangle]
pub extern "C" fn secp256k1_xonly_pubkey_from_privkey(
    privkey: *const u8,
    xonly_out: *mut u8,
) -> i32 {
    let secp = Secp256k1::new();

    unsafe {
        let privkey_bytes = slice::from_raw_parts(privkey, 32);
        let secret_key = match SecretKey::from_slice(privkey_bytes) {
            Ok(sk) => sk,
            Err(_) => return 0,
        };

        let keypair = Keypair::from_secret_key(&secp, &secret_key);
        let (xonly, _parity) = keypair.x_only_public_key();

        let xonly_slice = slice::from_raw_parts_mut(xonly_out, 32);
        xonly_slice.copy_from_slice(&xonly.serialize());
    }
    1
}

/// ECDH shared secret derivation (32 bytes)
/// Returns 1 on success, 0 on failure
#[no_mangle]
pub extern "C" fn secp256k1_ecdh(
    privkey: *const u8,
    pubkey: *const u8,
    pubkey_len: usize,
    secret_out: *mut u8,
) -> i32 {
    unsafe {
        let privkey_bytes = slice::from_raw_parts(privkey, 32);
        let pubkey_bytes = slice::from_raw_parts(pubkey, pubkey_len);

        let secret_key = match SecretKey::from_slice(privkey_bytes) {
            Ok(sk) => sk,
            Err(_) => return 0,
        };

        let public_key = match PublicKey::from_slice(pubkey_bytes) {
            Ok(pk) => pk,
            Err(_) => return 0,
        };

        // Compute ECDH using the ecdh module
        let shared_secret = ecdh::shared_secret_point(&public_key, &secret_key);

        let secret_slice = slice::from_raw_parts_mut(secret_out, 32);
        secret_slice.copy_from_slice(&shared_secret[..32]);
    }
    1
}

/// Schnorr sign a message (64 bytes signature)
/// Returns 1 on success, 0 on failure
#[no_mangle]
pub extern "C" fn secp256k1_schnorr_sign(
    privkey: *const u8,
    msg_hash: *const u8,
    sig_out: *mut u8,
) -> i32 {
    let secp = Secp256k1::new();

    unsafe {
        let privkey_bytes = slice::from_raw_parts(privkey, 32);
        let msg_bytes = slice::from_raw_parts(msg_hash, 32);

        let secret_key = match SecretKey::from_slice(privkey_bytes) {
            Ok(sk) => sk,
            Err(_) => return 0,
        };

        let keypair = Keypair::from_secret_key(&secp, &secret_key);
        let message = Message::from_digest_slice(msg_bytes).unwrap();
        let signature = secp.sign_schnorr(&message, &keypair);

        let sig_slice = slice::from_raw_parts_mut(sig_out, 64);
        sig_slice.copy_from_slice(&signature[..]);
    }
    1
}

/// Verify Schnorr signature
/// Returns 1 if valid, 0 if invalid
#[no_mangle]
pub extern "C" fn secp256k1_schnorr_verify(
    xonly_pubkey: *const u8,
    msg_hash: *const u8,
    signature: *const u8,
) -> i32 {
    let secp = Secp256k1::new();

    unsafe {
        let xonly_bytes = slice::from_raw_parts(xonly_pubkey, 32);
        let msg_bytes = slice::from_raw_parts(msg_hash, 32);
        let sig_bytes = slice::from_raw_parts(signature, 64);

        let xonly = match XOnlyPublicKey::from_slice(xonly_bytes) {
            Ok(xo) => xo,
            Err(_) => return 0,
        };

        let message = Message::from_digest_slice(msg_bytes).unwrap();
        let sig = match SchnorrSignature::from_slice(sig_bytes) {
            Ok(s) => s,
            Err(_) => return 0,
        };

        match secp.verify_schnorr(&sig, &message, &xonly) {
            Ok(_) => 1,
            Err(_) => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_generation() {
        let mut privkey = [0u8; 32];
        assert_eq!(secp256k1_privkey_generate(privkey.as_mut_ptr()), 1);
        assert_ne!(privkey, [0u8; 32]); // Should not be all zeros
    }

    #[test]
    fn test_pubkey_derivation() {
        let mut privkey = [0u8; 32];
        let mut pubkey = [0u8; 33];

        secp256k1_privkey_generate(privkey.as_mut_ptr());
        assert_eq!(
            secp256k1_pubkey_from_privkey(privkey.as_ptr(), pubkey.as_mut_ptr()),
            1
        );

        // Compressed pubkey should start with 0x02 or 0x03
        assert!(pubkey[0] == 0x02 || pubkey[0] == 0x03);
    }
}
