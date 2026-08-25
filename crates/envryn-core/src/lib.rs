// The vault core forbids unwrap/expect/panic: a panic here is a denial of the
// user's own data, and in a lock path it could leave the vault open. Tests are
// exempt -- there, a panic is the reporting mechanism.
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )
)]

//! Envryn vault core.
//!
//! Holds every security-relevant decision: key derivation, encryption,
//! storage, and vault state. Deliberately free of Tauri, of any UI framework,
//! and of any network client, so that it can be tested as a plain library and
//! so that the dependency graph itself enforces INV-010.

pub mod backup;
pub mod crypto;
pub mod error;
pub mod model;
pub mod platform;
pub mod storage;
pub mod sync;
pub mod vault;

pub use error::{Error, Result};
pub use vault::Vault;
