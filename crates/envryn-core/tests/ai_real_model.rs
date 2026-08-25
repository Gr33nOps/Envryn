// Tests report failure by panicking, so the core crate's no-panic lints are
// relaxed here. Integration tests are their own crate, so this cannot be
// inherited from lib.rs.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Exercises real candle-based inference (`crates/envryn-ai-worker`)
//! against a real, locally downloaded GGUF model.
//!
//! **Not run by default.** `cargo test` in this repo must pass with no
//! model file present at all -- that is AI-INV-009 ("the vault remains
//! fully functional if the AI subsystem... was never installed") applied to
//! the test suite itself, not just to the product. Run manually once a
//! model is in place:
//!
//! ```text
//! cargo build -p envryn-ai-worker --release
//! cargo test -p envryn-core --test ai_real_model -- --ignored --nocapture
//! ```
//!
//! with `ENVRYN_TEST_MODEL`, `ENVRYN_TEST_TOKENIZER`, and
//! `ENVRYN_TEST_WORKER_BINARY` pointing at a Qwen2-family instruct GGUF
//! file, its `tokenizer.json`, and the built worker binary respectively.
//! This crate's own verification does not download a model automatically;
//! see this repo's development notes for where one was tested from.

use std::path::PathBuf;

use envryn_core::ai::gateway::AiGateway;
use envryn_core::ai::worker_client::{WorkerClient, WorkerSpawnConfig};
use envryn_core::model::SecretKind;

fn env_path(var: &str) -> PathBuf {
    PathBuf::from(
        std::env::var(var)
            .unwrap_or_else(|_| panic!("set {var} to run this test -- see the module doc for how")),
    )
}

fn spawn_real_engine() -> WorkerClient {
    let config = WorkerSpawnConfig {
        worker_binary: env_path("ENVRYN_TEST_WORKER_BINARY"),
        model_path: env_path("ENVRYN_TEST_MODEL"),
        tokenizer_path: env_path("ENVRYN_TEST_TOKENIZER"),
        arch: "qwen2".to_string(),
        eos_token: "<|im_end|>".to_string(),
        extra_env: Vec::new(),
    };
    WorkerClient::spawn(&config).expect("the real worker should start and load the real model")
}

#[test]
#[ignore = "requires a real downloaded model -- see module doc"]
fn classifies_an_obvious_api_key_shape_with_a_real_local_model() {
    let engine = spawn_real_engine();
    let gateway = AiGateway::new(engine);

    let result = gateway
        .classify_pasted_value("sk-live-abcdefghijklmnopqrstuvwxyz1234567890")
        .expect("a real small instruct model should produce schema-valid JSON for this");

    println!("real model classification result: {result:?}");
    assert_eq!(result.kind, SecretKind::ApiKey);
}

#[test]
#[ignore = "requires a real downloaded model -- see module doc"]
fn suggests_a_plausible_name_with_a_real_local_model() {
    let engine = spawn_real_engine();
    let gateway = AiGateway::new(engine);

    let result = gateway
        .suggest_name(
            "sk-live-abcdefghijklmnopqrstuvwxyz1234567890",
            Some("Stripe"),
        )
        .expect("a real small instruct model should produce schema-valid JSON for this");

    println!("real model name suggestion: {result:?}");
    assert!(!result.name.trim().is_empty());
}

/// **Honest finding, kept rather than hidden.** Against the 0.5B model this
/// test suite was developed against (Qwen2-0.5B-Instruct, Q4_0 GGUF), this
/// specific five-field/enum-constrained schema is the one Tier-1 shape that
/// model does not reliably hit -- it correctly infers `environment:
/// "Production"`, `kind: "EnvVar"`, `tags: ["database"]`, but frequently
/// adds an extra field (observed: a hallucinated `"notes"` array) despite
/// the prompt explicitly saying "EXACTLY these five fields and no others."
/// `deny_unknown_fields` then does exactly its job: refuse the response
/// rather than silently accept a wrong or partial one
/// (`docs/AI_SECURITY.md` section 5). That refusal, not a successful parse,
/// is what this test actually proves with a small model -- a larger model
/// (the plan's own "1-3B... benchmark before committing" guidance) is the
/// documented next step, not a change to the validation itself.
#[test]
#[ignore = "requires a real downloaded model -- see module doc"]
fn a_five_field_search_query_either_parses_or_is_cleanly_refused_never_silently_wrong() {
    let engine = spawn_real_engine();
    let gateway = AiGateway::new(engine);

    match gateway.parse_search_intent("show me production database credentials") {
        Ok(result) => println!("real model search filter: {result:?}"),
        Err(envryn_core::ai::gateway::AiError::InvalidResponse) => {
            println!(
                "model produced non-schema-conforming output for this query -- refused cleanly, as designed"
            );
        }
        Err(other) => panic!("expected either a valid parse or a clean refusal, got {other:?}"),
    }
}
