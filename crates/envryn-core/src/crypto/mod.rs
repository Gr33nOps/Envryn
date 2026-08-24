//! Cryptographic primitives and the key hierarchy.
//!
//! Every algorithm Envryn uses is listed in docs/CRYPTOGRAPHY.md. No other
//! cryptographic implementation may enter the codebase without amending
//! docs/DEPENDENCY_POLICY.md.

pub mod aead;
pub mod fingerprint;
pub mod kdf;
pub mod keys;
