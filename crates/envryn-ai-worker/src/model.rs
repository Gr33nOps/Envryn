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
//! **Known gap.** There is no grammar-constrained decoding (GBNF or
//! equivalent) here. `docs/AI_SECURITY.md` section 5 describes the model as
//! "physically unable to emit anything that is not schema-valid" via
//! llama.cpp's GBNF support; this candle-based engine does not implement an
//! equivalent token-level constraint. Structured output relies entirely on
//! prompting plus the strict `deny_unknown_fields` deserialisation already
//! enforced in `envryn_core::ai::gateway` -- a real second layer, but not
//! the two-layer design the docs describe. Recorded here rather than
//! silently implemented as less than what the docs claim.

use std::path::Path;
use std::sync::Mutex;

use candle_core::quantized::gguf_file;
use candle_core::{Device, Tensor};
use candle_transformers::generation::{LogitsProcessor, Sampling};
use candle_transformers::models::quantized_qwen2::ModelWeights as Qwen2Weights;
use tokenizers::Tokenizer;

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

        Ok(Self {
            model: Mutex::new(model),
            tokenizer,
            device,
            eos_token_id,
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
}

fn forward(model: &mut ModelKind, input: &Tensor, index_pos: usize) -> candle_core::Result<Tensor> {
    match model {
        ModelKind::Qwen2(m) => m.forward(input, index_pos),
    }
}
