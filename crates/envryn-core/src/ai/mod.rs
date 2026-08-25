//! The AI subsystem: a permission gateway around an optional local model.
//!
//! `docs/AI_SECURITY.md` and `docs/AI_DATA_ACCESS.md` are the normative
//! specifications this module implements; where the two disagree, the docs
//! win and this code has a bug. The governing idea, restated here because it
//! shapes every type below: every AI access rule is enforced by something a
//! contributor would have to actively fight -- a private field, a missing
//! dependency, a failing test -- never by a rule that lives only in a
//! comment.
//!
//! ```text
//! operations   AiOperation -- one variant per capability, each with a fixed
//!              exposure level (docs/AI_DATA_ACCESS.md section 1)
//! gateway      the only module that can construct a SanitizedPrompt;
//!              resolves operations, enforces budgets, calls the engine
//! engine       LocalAiEngine trait -- what a model backend must implement
//! classify     deterministic (non-AI) classification -- runs first, and is
//!              the entire feature with no model installed
//! worker_client a LocalAiEngine that talks to crates/envryn-ai-worker over
//!              loopback TCP; lives here (not in src-tauri) so it can be
//!              tested as a plain library, matching sync/'s precedent
//! ```
//!
//! **AI-INV-009, restated structurally:** nothing in `crate::vault` or
//! `crate::storage` imports anything from `crate::ai`, and nothing in this
//! module can reach a `VaultMasterKey`, a `SymmetricKey`, or a `Store`
//! directly -- every path from here to vault data goes through plain owned
//! values a caller already had to hand over explicitly. The vault is
//! usable, tested, and fully functional with this entire module deleted.

pub mod budgets;
pub mod classify;
pub mod engine;
pub mod gateway;
pub mod model_download;
pub mod operations;
pub mod schemas;
pub mod worker_client;

pub use engine::{EngineError, LocalAiEngine};
pub use gateway::{AiError, AiGateway, SanitizedPrompt};
pub use operations::{AiOperation, ExposureLevel};
