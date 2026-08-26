#![no_main]

use libfuzzer_sys::fuzz_target;

// Real target: `aead::open` / `Sealed::from_bytes` are the boundary that
// receives every ciphertext this app ever decrypts -- vault records read
// from SQLite, sync payloads received from a peer, and backup contents.
// Envryn's own AEAD test suite (crates/envryn-core/src/crypto/aead.rs)
// already covers the specific cases (tampered tag, tampered nonce, wrong
// key, truncated blob) by construction; this explores the much larger space
// of arbitrary byte layouts a real corrupted file or hostile peer could send
// -- the property under test is simply "never panics, always returns a
// `Result`", since a panic here would be a denial-of-service on every other
// call site that assumes `open` is infallible-but-fails-safely.
use envryn_core::crypto::aead::{open, Sealed};
use envryn_core::crypto::keys::SymmetricKey;

fuzz_target!(|data: &[u8]| {
    let key = SymmetricKey::from_bytes([0x42u8; 32]);
    if let Ok(sealed) = Sealed::from_bytes(data.to_vec()) {
        let _ = open(&key, &sealed, b"fuzz-aad");
    }
});
