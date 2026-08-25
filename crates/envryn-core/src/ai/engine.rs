//! What a local model backend must implement.
//!
//! Deliberately the *only* way [`crate::ai::gateway::AiGateway`] can reach a
//! model. Swapping the backend (a different worker, a different model
//! runtime, a test double) never touches gateway logic.

use crate::ai::gateway::SanitizedPrompt;

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    /// No worker process is running, or it could not be reached. Vault
    /// operations never depend on this succeeding (AI-INV-009) --
    /// deterministic classification (`crate::ai::classify`) already covers
    /// the common case with no model at all.
    #[error("the local AI model is not available")]
    Unavailable,
    #[error("the local AI model did not respond in time")]
    Timeout,
    /// The worker responded, but not with something `AiGateway` can use --
    /// malformed framing, a token that failed auth, or output that did not
    /// parse. Never distinguished further to the caller; the detail (if any)
    /// goes to a log line carrying no prompt content, never to the UI.
    #[error("the local AI model did not return a usable response")]
    Malformed,
}

/// Blocking by design, matching `crate::sync`'s transport and protocol
/// modules -- callers (the IPC layer) run this behind `spawn_blocking`
/// rather than the trait itself pulling in an async runtime dependency this
/// crate would otherwise not need.
pub trait LocalAiEngine: Send + Sync {
    /// Run one completion. `max_tokens` bounds the response length; an
    /// engine that ignores it is still capped by the caller discarding
    /// anything returned beyond the bound (`crate::ai::gateway`).
    fn complete(&self, prompt: &SanitizedPrompt, max_tokens: u32) -> Result<String, EngineError>;
}
