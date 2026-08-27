//! Loads a quantized GGUF model and runs greedy, structured-output-oriented
//! generation. CPU-only (`candle_core::Device::Cpu`) -- this process must
//! run on whatever machine Envryn is installed on, GPU or not
//! (specification section 51, "usable without a GPU").
//!
//! **Scope, stated plainly.** This targets one model family at a time,
//! selected by the parent process at spawn time via `--arch`. Only the
//! Qwen2 GGUF family and ChatML-style instruct formatting
//! (`<|im_start|>role\n...<|im_end|>`) have been exercised end-to-end (see
//! this crate's `tests/`); adding another architecture means adding another
//! `match` arm here, not a rewrite. Sampling is deterministic (greedy,
//! `Sampling::ArgMax`) with a light repetition penalty -- appropriate for
//! short, schema-constrained JSON output, where drift from randomness is a
//! cost with no corresponding benefit here (there is no "creative" AI
//! feature in Envryn's Tier 1 set).
//!
//! **Grammar-constrained decoding, for the one schema that has needed it so
//! far.** `docs/AI_SECURITY.md` section 5 used to describe this as a known
//! gap: llama.cpp's GBNF makes the model "physically unable to emit
//! anything that is not schema-valid," and this candle-based engine had no
//! equivalent. [`generate_constrained`](Engine::generate_constrained) closes
//! it for `ClassificationOutput` (see [`crate::constrained`] for the
//! mechanism and why that one schema first) by masking every token whose
//! text would not extend a valid prefix of that grammar, at every
//! generation step. `generate` (unconstrained) remains for every other
//! operation, which still relies on prompting plus
//! `envryn_core::ai::gateway`'s strict `deny_unknown_fields` deserialisation
//! -- a real second layer on its own, just not the same structural guarantee
//! as the grammar mask.

use std::path::Path;
use std::sync::Mutex;

use candle_core::quantized::gguf_file;
use candle_core::{Device, Tensor};
use candle_transformers::generation::{LogitsProcessor, Sampling};
use candle_transformers::models::quantized_qwen2::ModelWeights as Qwen2Weights;
use tokenizers::Tokenizer;

use crate::constrained::{GrammarState, JsonGrammar};

const REPEAT_PENALTY: f32 = 1.15;
const REPEAT_LAST_N: usize = 64;
const SYSTEM_PROMPT: &str =
    "You are a precise local assistant. Output only what is requested, nothing else.";

enum ModelKind {
    Qwen2(Qwen2Weights),
}

pub struct Engine {
    model: Mutex<ModelKind>,
    tokenizer: Tokenizer,
    device: Device,
    eos_token_id: u32,
    /// Every vocabulary id's decoded text, computed once at load time.
    /// [`Engine::generate_constrained`] needs this exactly once per
    /// generation *step* (checking every candidate token against the
    /// grammar), so paying the cost of decoding the whole vocabulary
    /// individually is worth it up front rather than repeated per step --
    /// this is the only O(vocab) cost, not a per-token one.
    vocab_text: Vec<String>,
}

#[derive(Debug)]
pub struct EngineLoadError(pub String);

impl std::fmt::Display for EngineLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

fn to_load_err(e: impl std::fmt::Display) -> EngineLoadError {
    EngineLoadError(e.to_string())
}

impl Engine {
    /// `arch` selects which GGUF architecture to load with -- see the
    /// module doc's scope note. `eos_token` is the literal end-of-turn
    /// token string in the tokenizer's vocabulary (e.g. `<|im_end|>` for
    /// ChatML-formatted models).
    pub fn load(
        arch: &str,
        model_path: &Path,
        tokenizer_path: &Path,
        eos_token: &str,
    ) -> Result<Self, EngineLoadError> {
        let device = Device::Cpu;

        let mut file = std::fs::File::open(model_path).map_err(to_load_err)?;
        let content = gguf_file::Content::read(&mut file).map_err(to_load_err)?;
        let model = match arch {
            "qwen2" => ModelKind::Qwen2(
                Qwen2Weights::from_gguf(content, &mut file, &device).map_err(to_load_err)?,
            ),
            other => {
                return Err(EngineLoadError(format!(
                    "unsupported model architecture '{other}' -- only 'qwen2' is implemented"
                )))
            }
        };

        let tokenizer =
            Tokenizer::from_file(tokenizer_path).map_err(|e| EngineLoadError(e.to_string()))?;
        let eos_token_id = *tokenizer
            .get_vocab(true)
            .get(eos_token)
            .ok_or_else(|| EngineLoadError(format!("tokenizer has no '{eos_token}' token")))?;

        let vocab_size = tokenizer.get_vocab_size(true);
        let vocab_text: Vec<String> = (0..vocab_size as u32)
            .map(|id| tokenizer.decode(&[id], false).unwrap_or_default())
            .collect();

        Ok(Self {
            model: Mutex::new(model),
            tokenizer,
            device,
            eos_token_id,
            vocab_text,
        })
    }

    /// Format, tokenize, and greedily generate up to `max_tokens` tokens,
    /// stopping at the end-of-turn token. Returns the decoded completion
    /// text (special tokens stripped).
    pub fn generate(&self, prompt: &str, max_tokens: u32) -> Result<String, String> {
        let formatted = format!(
            "<|im_start|>system\n{SYSTEM_PROMPT}<|im_end|>\n<|im_start|>user\n{prompt}<|im_end|>\n<|im_start|>assistant\n"
        );
        let encoding = self
            .tokenizer
            .encode(formatted, true)
            .map_err(|e| e.to_string())?;
        let prompt_tokens = encoding.get_ids();
        if prompt_tokens.is_empty() {
            return Err("empty prompt after tokenization".to_string());
        }

        let mut model = self
            .model
            .lock()
            .map_err(|_| "model lock poisoned by a previous panic".to_string())?;
        let mut logits_processor = LogitsProcessor::from_sampling(rand::random(), Sampling::ArgMax);
        let mut all_tokens: Vec<u32> = Vec::new();

        let mut next_token = {
            let input = Tensor::new(prompt_tokens, &self.device)
                .and_then(|t| t.unsqueeze(0))
                .map_err(|e| e.to_string())?;
            let logits = forward(&mut model, &input, 0).map_err(|e| e.to_string())?;
            let logits = logits.squeeze(0).map_err(|e| e.to_string())?;
            logits_processor
                .sample(&logits)
                .map_err(|e| e.to_string())?
        };
        all_tokens.push(next_token);

        let to_sample = max_tokens.saturating_sub(1) as usize;
        for index in 0..to_sample {
            if next_token == self.eos_token_id {
                break;
            }
            let input = Tensor::new(&[next_token], &self.device)
                .and_then(|t| t.unsqueeze(0))
                .map_err(|e| e.to_string())?;
            let logits = forward(&mut model, &input, prompt_tokens.len() + index)
                .map_err(|e| e.to_string())?;
            let logits = logits.squeeze(0).map_err(|e| e.to_string())?;
            let start_at = all_tokens.len().saturating_sub(REPEAT_LAST_N);
            let logits = candle_transformers::utils::apply_repeat_penalty(
                &logits,
                REPEAT_PENALTY,
                &all_tokens[start_at..],
            )
            .map_err(|e| e.to_string())?;
            next_token = logits_processor
                .sample(&logits)
                .map_err(|e| e.to_string())?;
            if next_token == self.eos_token_id {
                break;
            }
            all_tokens.push(next_token);
        }

        self.tokenizer
            .decode(&all_tokens, true)
            .map_err(|e| e.to_string())
    }

    /// As [`Engine::generate`], but every candidate token at every step is
    /// checked against `grammar` first (see [`crate::constrained`]) and
    /// masked out (`-inf`) if choosing it could not lead to schema-valid
    /// JSON -- the model is structurally unable to pick it, not merely
    /// discouraged by the prompt. Stops on the grammar reaching its final
    /// `}` (not only on the model's own EOS token, which a constrained
    /// model has no reason to emit mid-JSON but also no obligation to emit
    /// promptly once the JSON is actually complete).
    pub fn generate_constrained(
        &self,
        prompt: &str,
        max_tokens: u32,
        grammar: &JsonGrammar,
    ) -> Result<String, String> {
        let formatted = format!(
            "<|im_start|>system\n{SYSTEM_PROMPT}<|im_end|>\n<|im_start|>user\n{prompt}<|im_end|>\n<|im_start|>assistant\n"
        );
        let encoding = self
            .tokenizer
            .encode(formatted, true)
            .map_err(|e| e.to_string())?;
        let prompt_tokens = encoding.get_ids();
        if prompt_tokens.is_empty() {
            return Err("empty prompt after tokenization".to_string());
        }

        let mut model = self
            .model
            .lock()
            .map_err(|_| "model lock poisoned by a previous panic".to_string())?;
        let mut logits_processor = LogitsProcessor::from_sampling(rand::random(), Sampling::ArgMax);
        let mut all_tokens: Vec<u32> = Vec::new();
        let mut grammar_state = grammar.start();

        let mut next_token = {
            let input = Tensor::new(prompt_tokens, &self.device)
                .and_then(|t| t.unsqueeze(0))
                .map_err(|e| e.to_string())?;
            let logits = forward(&mut model, &input, 0).map_err(|e| e.to_string())?;
            let logits = logits.squeeze(0).map_err(|e| e.to_string())?;
            let masked = self.mask_to_grammar(&logits, &grammar_state)?;
            logits_processor
                .sample(&masked)
                .map_err(|e| e.to_string())?
        };
        grammar_state = self.advance_grammar(&grammar_state, next_token)?;
        all_tokens.push(next_token);

        let to_sample = max_tokens.saturating_sub(1) as usize;
        for index in 0..to_sample {
            if grammar_state.is_complete() || next_token == self.eos_token_id {
                break;
            }
            let input = Tensor::new(&[next_token], &self.device)
                .and_then(|t| t.unsqueeze(0))
                .map_err(|e| e.to_string())?;
            let logits = forward(&mut model, &input, prompt_tokens.len() + index)
                .map_err(|e| e.to_string())?;
            let logits = logits.squeeze(0).map_err(|e| e.to_string())?;
            let start_at = all_tokens.len().saturating_sub(REPEAT_LAST_N);
            let logits = candle_transformers::utils::apply_repeat_penalty(
                &logits,
                REPEAT_PENALTY,
                &all_tokens[start_at..],
            )
            .map_err(|e| e.to_string())?;
            let masked = self.mask_to_grammar(&logits, &grammar_state)?;
            next_token = logits_processor
                .sample(&masked)
                .map_err(|e| e.to_string())?;
            grammar_state = self.advance_grammar(&grammar_state, next_token)?;
            all_tokens.push(next_token);
        }

        self.tokenizer
            .decode(&all_tokens, true)
            .map_err(|e| e.to_string())
    }

    /// Advance the grammar by the token the sampler just chose.
    ///
    /// **This used to index `vocab_text` directly, which could panic and
    /// take the whole worker process down.** A model's logits vector is
    /// sized to its own vocabulary, which is routinely *larger* than the
    /// tokenizer's (Qwen2.5 pads to 151936 against 151643 real tokens) --
    /// so a sampled id is not guaranteed to be a valid `vocab_text` index.
    /// `mask_to_grammar` sets those padded positions to `-inf`, which makes
    /// the panic unreachable in practice today, but "unreachable because
    /// another function maintains an invariant" is exactly the kind of
    /// coupling that breaks silently later -- and the failure mode was a
    /// hard process abort mid-inference, not a recoverable error. A missing
    /// entry is now the same clean `Err` any other bad model output
    /// produces, which the client surfaces as a normal AI error.
    fn advance_grammar(&self, state: &GrammarState, token: u32) -> Result<GrammarState, String> {
        advance_grammar_with_vocab(&self.vocab_text, state, token)
    }

    /// Set every token's logit to `-inf` except those whose decoded text is
    /// a valid continuation of `state` -- see [`crate::constrained`].
    fn mask_to_grammar(&self, logits: &Tensor, state: &GrammarState) -> Result<Tensor, String> {
        let mut values = logits.to_vec1::<f32>().map_err(|e| e.to_string())?;
        for (id, value) in values.iter_mut().enumerate() {
            let Some(text) = self.vocab_text.get(id) else {
                *value = f32::NEG_INFINITY;
                continue;
            };
            if text.is_empty() || state.try_advance(text).is_none() {
                *value = f32::NEG_INFINITY;
            }
        }
        Tensor::new(values, logits.device()).map_err(|e| e.to_string())
    }
}

/// Look up a sampled token's text and advance the grammar by it.
///
/// Split out from [`Engine::advance_grammar`] as a free function purely so
/// it is testable: the method needs a loaded `Engine` (a real
/// multi-hundred-megabyte model file), while the out-of-range case this
/// guards against is a property of the *lookup*, not of the model.
fn advance_grammar_with_vocab(
    vocab_text: &[String],
    state: &GrammarState,
    token: u32,
) -> Result<GrammarState, String> {
    let text = vocab_text
        .get(token as usize)
        .ok_or_else(|| "model sampled a token outside the tokenizer vocabulary".to_string())?;
    state
        .try_advance(text)
        .ok_or_else(|| "grammar mask selected a token it should have excluded".to_string())
}

fn forward(model: &mut ModelKind, input: &Tensor, index_pos: usize) -> candle_core::Result<Tensor> {
    match model {
        ModelKind::Qwen2(m) => m.forward(input, index_pos),
    }
}

// Tests report failure by panicking, so the crate's no-panic lints are
// relaxed here -- matching the same scoped pattern
// `crates/envryn-core/tests/*.rs` use at their file level.
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use super::*;

    const KIND_VARIANTS: &[&str] = &[
        "ApiKey", "Token", "EnvVar", "Database", "Ssh", "OAuth", "Webhook", "Note", "Custom",
    ];

    /// The actual crash regression. A model's logits vector is sized to its
    /// own padded vocabulary, which is larger than the tokenizer's real one
    /// (Qwen2.5: 151936 vs 151643), so a sampled id can legitimately fall
    /// past the end of `vocab_text`. This used to be a bare slice index --
    /// an out-of-bounds panic that aborted the whole worker process
    /// mid-inference, which the app then saw as the AI subsystem dying
    /// rather than as a request that failed. It must be an ordinary `Err`.
    #[test]
    fn a_token_id_past_the_end_of_the_vocabulary_errors_instead_of_panicking() {
        let grammar = JsonGrammar::classification_output(KIND_VARIANTS);
        let vocab: Vec<String> = vec!["{".to_string(), "\"".to_string()];

        let result = advance_grammar_with_vocab(&vocab, &grammar.start(), 151_935);

        assert!(
            result.is_err(),
            "an out-of-range token id must be a clean error, not a panic"
        );
    }

    /// The in-range path still works -- the bounds check must not have
    /// turned every advance into a failure.
    #[test]
    fn an_in_range_token_still_advances_the_grammar() {
        let grammar = JsonGrammar::classification_output(KIND_VARIANTS);
        let vocab: Vec<String> = vec!["{".to_string()];

        let result = advance_grammar_with_vocab(&vocab, &grammar.start(), 0);

        assert!(result.is_ok(), "a valid opening brace must advance cleanly");
    }

    /// An in-range token whose text the grammar forbids is also a clean
    /// error, not a panic -- the two failure modes stay distinguishable in
    /// the message but identical in kind.
    #[test]
    fn an_in_range_but_grammar_invalid_token_errors_cleanly() {
        let grammar = JsonGrammar::classification_output(KIND_VARIANTS);
        let vocab: Vec<String> = vec!["definitely not valid json".to_string()];

        let result = advance_grammar_with_vocab(&vocab, &grammar.start(), 0);

        assert!(result.is_err());
    }

    /// Not run by default: requires a real downloaded model, same convention
    /// as `crates/envryn-core/tests/ai_real_model.rs`. This is the actual
    /// proof that `generate_constrained` produces schema-valid JSON from a
    /// real model -- unit tests on `GrammarState` alone prove the state
    /// machine's logic, not that masking real per-step logits against a
    /// real vocabulary and a real forward pass behaves the same way. Run:
    ///
    /// ```text
    /// ENVRYN_TEST_MODEL=... ENVRYN_TEST_TOKENIZER=... \
    ///   cargo test -p envryn-ai-worker --release generate_constrained_always_produces_valid_json -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "requires a real downloaded model -- see crates/envryn-core/tests/ai_real_model.rs's module doc for how"]
    fn generate_constrained_always_produces_valid_json() {
        let model_path = std::env::var("ENVRYN_TEST_MODEL")
            .unwrap_or_else(|_| panic!("set ENVRYN_TEST_MODEL to run this test"));
        let tokenizer_path = std::env::var("ENVRYN_TEST_TOKENIZER")
            .unwrap_or_else(|_| panic!("set ENVRYN_TEST_TOKENIZER to run this test"));
        let engine = Engine::load(
            "qwen2",
            std::path::Path::new(&model_path),
            std::path::Path::new(&tokenizer_path),
            "<|im_end|>",
        )
        .expect("engine should load against a real model");

        let grammar = JsonGrammar::classification_output(KIND_VARIANTS);
        // Real, adversarial-ish prompts -- not just one easy case -- each
        // checked against the same real property: valid JSON, exactly the
        // three known fields, `kind` one of the real enum variants,
        // `confidence` within [0, 1].
        let prompts = [
            "Classify this credential and reply with the required JSON only: sk-live-abcdefghijklmnopqrstuvwxyz1234567890",
            "Classify this credential and reply with the required JSON only: ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789",
            "Classify this credential and reply with the required JSON only: postgres://user:pass@localhost:5432/db",
            "Classify this credential and reply with the required JSON only: just some random unrecognisable text",
        ];

        for prompt in prompts {
            let output = engine
                .generate_constrained(prompt, 64, &grammar)
                .unwrap_or_else(|e| panic!("generation failed for {prompt:?}: {e}"));
            let parsed: serde_json::Value = serde_json::from_str(&output).unwrap_or_else(|e| {
                panic!("output was not valid JSON for {prompt:?}: {e}\noutput: {output}")
            });
            let obj = parsed.as_object().expect("output must be a JSON object");
            assert_eq!(
                obj.len(),
                3,
                "output must have exactly the 3 schema fields, got: {output}"
            );
            let kind = obj
                .get("kind")
                .and_then(|v| v.as_str())
                .expect("kind must be a string");
            assert!(
                KIND_VARIANTS.contains(&kind),
                "kind {kind:?} must be one of the real SecretKind variants"
            );
            assert!(
                obj.get("provider")
                    .is_some_and(|v| v.is_null() || v.is_string()),
                "provider must be null or a string, got: {output}"
            );
            let confidence = obj
                .get("confidence")
                .and_then(|v| v.as_f64())
                .expect("confidence must be a number");
            assert!(
                (0.0..=1.0).contains(&confidence),
                "confidence {confidence} must be within [0, 1]"
            );
            println!("prompt: {prompt}\n  -> {output}");
        }
    }
}

#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
#[cfg(test)]
mod diagnostic {
    use super::*;

    /// Diagnostic, not an assertion: prints exactly what the real model
    /// emits for the `.env` name-classification prompt so a schema mismatch
    /// can be read off directly instead of guessed at from an
    /// `InvalidResponse`. Uses fake variable names only.
    ///
    /// This is how the real defect behind "`.env` import produces no AI
    /// suggestions" was found rather than guessed: the model answered with a
    /// bare top-level array (not the requested wrapper object) *and* one
    /// invented `kind` value, which strict parsing turned into a total loss
    /// of all three classifications. Kept because the next schema mismatch
    /// will be diagnosed the same way. Never asserts, so it cannot fail for
    /// a reason that is really just the model being a small model.
    ///
    /// The prompt below is a **copy** of `envryn_core::ai::gateway`'s
    /// `CLASSIFY_ENV_NAMES_PROMPT`, not a reference to it -- this crate must
    /// not depend on `envryn-core` (AI-INV-001/002/004/005). If that prompt
    /// changes and this diagnostic stops reproducing the real behaviour,
    /// re-copy it; nothing enforces the correspondence.
    #[test]
    #[ignore = "diagnostic -- requires a real downloaded model"]
    fn show_raw_env_name_classification_output() {
        let model_path = std::env::var("ENVRYN_TEST_MODEL").expect("set ENVRYN_TEST_MODEL");
        let tok_path = std::env::var("ENVRYN_TEST_TOKENIZER").expect("set ENVRYN_TEST_TOKENIZER");
        let engine = Engine::load(
            "qwen2",
            std::path::Path::new(&model_path),
            std::path::Path::new(&tok_path),
            "<|im_end|>",
        )
        .expect("engine should load");

        let prompt = "You classify environment-variable NAMES only (never \
    values) by what kind of credential they likely hold. Output ONLY a JSON object with field \
    \"names\": an array of objects, each with \"name\" (the input name, verbatim) and \"kind\" \
    (one of \"ApiKey\",\"Token\",\"EnvVar\",\"Database\",\"Ssh\",\"OAuth\",\"Webhook\",\"Note\",\"Custom\"). \
    One name per line follows, delimited as untrusted data.\n\n\
    <<<UNTRUSTED_VAULT_DATA>>>\nDATABASE_URL\nSTRIPE_SECRET_KEY\nGITHUB_TOKEN\n<<<END_UNTRUSTED_VAULT_DATA>>>";

        let out = engine
            .generate(prompt, 1024)
            .expect("generation should work");
        println!("=== RAW MODEL OUTPUT START ===");
        println!("{out}");
        println!("=== RAW MODEL OUTPUT END ===");
    }
}
