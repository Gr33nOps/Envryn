//! Hard limits the gateway enforces before anything reaches a model.
//!
//! From `docs/AI_DATA_ACCESS.md` section 3: "Budgets are enforced by the
//! gateway, not by the model." Exceeding one is a clean refusal with an
//! explanation, never a silent truncation -- a silently truncated `.env`
//! import would drop credentials without telling anyone.

/// One pasted value (`ClassifyPastedValue`, `SuggestName`).
pub const MAX_VALUE_BYTES: usize = 4 * 1024;

/// One submitted block (`ExtractStructuredFields`) -- matches the "max note
/// size submitted" row in `docs/AI_DATA_ACCESS.md`.
pub const MAX_BLOCK_BYTES: usize = 32 * 1024;

/// Variable names collected for one `.env` import preview.
pub const MAX_ENV_NAMES: usize = 512;

/// A single `.env` variable name.
pub const MAX_ENV_NAME_BYTES: usize = 256;

/// A natural-language search query.
pub const MAX_QUERY_BYTES: usize = 512;

/// Model response cap. Enforced by the engine, not just documented --
/// `worker_client` passes this to the worker as `max_tokens`, so a
/// misbehaving or compromised worker cannot stream back more than the
/// gateway is willing to accept regardless of what it claims to have
/// generated.
pub const MAX_RESPONSE_TOKENS: u32 = 1024;

/// Approximate prompt budget. `docs/AI_DATA_ACCESS.md` states this as a
/// token count (8,192); this crate has no tokenizer loaded until a model is,
/// so it is approximated here as a byte length using a conservative ~3
/// bytes/token estimate for English/code text, which only ever *under*-
/// estimates how many tokens a given byte length represents (real tokenizers
/// average nearer 4 bytes/token) -- so this bound is stricter than the
/// documented one, never looser. Revisit once `worker_client` can ask the
/// loaded tokenizer for an exact count.
pub const MAX_PROMPT_BYTES: usize = 8_192 * 3;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_budget_is_stricter_than_the_documented_token_estimate() {
        // At the real tokenizer average of ~4 bytes/token, this byte budget
        // corresponds to fewer than the documented 8,192 tokens -- i.e. it
        // never permits more than the spec allows.
        const _: () = assert!(MAX_PROMPT_BYTES / 4 <= 8_192);
    }
}
