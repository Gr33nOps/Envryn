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

/// Identifies which known output shape a completion must conform to, for an
/// engine that can enforce it structurally (grammar-constrained decoding --
/// see `envryn-ai-worker`'s `constrained` module) rather than only through
/// prompting. Deliberately a closed, growing-as-needed set rather than an
/// arbitrary schema description: each variant corresponds to one real
/// grammar someone has actually built, not a promise every schema in
/// `crate::ai::schemas` is covered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaKind {
    /// No structural constraint requested -- prompting plus the caller's own
    /// `deny_unknown_fields` deserialisation is the only enforcement (see
    /// `docs/AI_SECURITY.md` section 5).
    Unconstrained,
    /// `crate::ai::schemas::ClassificationOutput` -- the one schema with a
    /// real grammar built for it so far, because it is the one Tier-1
    /// feature actually wired to the UI.
    ClassificationOutput,
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

    /// As [`LocalAiEngine::complete`], but naming which known schema the
    /// response must conform to. The default implementation ignores
    /// `_schema` and simply calls `complete` -- an engine with no stronger
    /// guarantee to offer (every test double, and any future engine that
    /// never implements constrained decoding) still works correctly, it
    /// just does not get the structural guarantee a real grammar provides.
    /// [`crate::ai::worker_client::WorkerClient`] is the one real override.
    fn complete_for_schema(
        &self,
        prompt: &SanitizedPrompt,
        max_tokens: u32,
        _schema: SchemaKind,
    ) -> Result<String, EngineError> {
        self.complete(prompt, max_tokens)
    }
}
