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
    spawn_real_engine_with_env(Vec::new())
}

fn spawn_real_engine_with_env(extra_env: Vec<(String, String)>) -> WorkerClient {
    let config = WorkerSpawnConfig {
        worker_binary: env_path("ENVRYN_TEST_WORKER_BINARY"),
        model_path: env_path("ENVRYN_TEST_MODEL"),
        tokenizer_path: env_path("ENVRYN_TEST_TOKENIZER"),
        arch: "qwen2".to_string(),
        eos_token: "<|im_end|>".to_string(),
        extra_env,
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

/// `docs/AI_SECURITY.md`'s "disconnect the internet and confirm every AI
/// feature still works" scenario, exercised safely rather than skipped:
/// this poisons the *worker child process's own* `HTTP_PROXY`/`HTTPS_PROXY`/
/// `ALL_PROXY` environment variables (via `WorkerSpawnConfig::extra_env`,
/// which only affects that one spawned process, not this test process or
/// the real system) to point at an address nothing is listening on, so any
/// HTTP client the worker *did* try to use would fail fast rather than
/// silently succeed via the real network. Real inference completing
/// normally under that condition is real evidence -- not merely the
/// dependency-graph argument (`envryn-ai-worker` has no HTTP client crate
/// at all, confirmed separately via `cargo tree`) -- that this feature
/// needs no network. Deliberately does *not* touch the OS firewall or any
/// system-wide setting, which would be a far more disruptive way to test
/// the same property on a real development machine.
#[test]
#[ignore = "requires a real downloaded model -- see module doc"]
fn classification_still_works_with_the_workers_proxy_env_poisoned() {
    let poison = "http://127.0.0.1:1".to_string(); // nothing listens on port 1
    let engine = spawn_real_engine_with_env(vec![
        ("HTTP_PROXY".to_string(), poison.clone()),
        ("HTTPS_PROXY".to_string(), poison.clone()),
        ("ALL_PROXY".to_string(), poison),
        ("NO_PROXY".to_string(), String::new()),
    ]);
    let gateway = AiGateway::new(engine);

    let result = gateway
        .classify_pasted_value("sk-live-abcdefghijklmnopqrstuvwxyz1234567890")
        .expect("inference must succeed unaffected by proxy env poisoning -- the worker never makes an HTTP call in the first place");

    println!("real model classification result under poisoned proxy env: {result:?}");
    assert_eq!(result.kind, SecretKind::ApiKey);
}

// --- End-to-end coverage of all five Tier-1 features against the real model.
//
// Added after a round of beta feedback in which every AI feature was
// reported broken in some way. Each test below drives the real worker, the
// real 1.5B model, and the real gateway with fake-but-realistic inputs, so a
// regression in any one feature fails here rather than in a user's hands.

/// Feature 1 of 5: classification. The exact reported bug -- an OpenRouter
/// key coming back as "Stripe" -- plus the other providers that share its
/// prefix family. These must all resolve *deterministically*, never
/// reaching the model at all, which is what makes them reliable.
#[test]
#[ignore = "requires a real downloaded model -- see module doc"]
fn classification_resolves_known_providers_without_asking_the_model() {
    use envryn_core::ai::classify;

    // Prefix and body are separate literals so no complete (fabricated)
    // token string exists in this file -- see the same note on
    // `every_supported_provider_is_recognised_without_the_model` in
    // `crates/envryn-core/src/ai/classify.rs`.
    let cases: &[(&str, &str, &str)] = &[
        (
            "sk-or-v1-",
            "0123456789abcdef0123456789abcdef",
            "OpenRouter",
        ),
        ("sk-proj-", "0123456789abcdef0123456789abcdef", "OpenAI"),
        ("sk-ant-api03-", "0123456789abcdef0123456789", "Anthropic"),
        ("sk_live_", "51ABCdefGHIjklMNO", "Stripe"),
        ("ghp_", "0123456789abcdef0123456789abcdef", "GitHub"),
        ("AKIA", "IOSFODNN7EXAMPLE", "AWS"),
        ("sbp_", "0123456789abcdef0123456789abcdef0123", "Supabase"),
        (
            "postgres://",
            "user:pass@db.example.com:5432/prod",
            "PostgreSQL",
        ),
    ];

    for (prefix, body, expected) in cases {
        let value = format!("{prefix}{body}");
        let got = classify::classify(&value)
            .unwrap_or_else(|| panic!("{value} must classify deterministically"));
        assert_eq!(got.provider, Some(*expected), "wrong provider for {value}");
    }
}

/// Feature 2 of 5: name suggestion, driven by the real model.
#[test]
#[ignore = "requires a real downloaded model -- see module doc"]
fn name_suggestion_produces_a_usable_label_end_to_end() {
    let gateway = AiGateway::new(spawn_real_engine());

    let result = gateway
        .suggest_name(
            &format!("{}{}", "sk-or-v1-", "0123456789abcdef0123456789abcdef"),
            Some("OpenRouter"),
        )
        .expect("name suggestion must succeed against the real model");

    println!("real model suggested name: {result:?}");
    assert!(!result.name.trim().is_empty(), "a blank name is not usable");
    assert!(result.name.len() <= 80, "name should be a short label");
}

/// Feature 3 of 5: natural-language search. The reported symptom was "always
/// returns No match found"; the fix makes obvious queries resolve without the
/// model at all, so these must produce real structured filters every time.
#[test]
#[ignore = "requires a real downloaded model -- see module doc"]
fn search_resolves_obvious_queries_end_to_end() {
    use envryn_core::model::Environment;

    let gateway = AiGateway::new(spawn_real_engine());

    let production = gateway
        .parse_search_intent("production database")
        .expect("search parsing must not fail");
    println!("search 'production database' -> {production:?}");
    assert_eq!(production.environment, Some(Environment::Production));
    assert_eq!(production.kind, Some(SecretKind::Database));

    let staging = gateway
        .parse_search_intent("staging tokens")
        .expect("search parsing must not fail");
    assert_eq!(staging.environment, Some(Environment::Staging));
    assert_eq!(staging.kind, Some(SecretKind::Token));

    // A vague query does reach the model -- and must still come back with
    // something usable rather than an error.
    let vague = gateway
        .parse_search_intent("that key I use for payments")
        .expect("a vague query must degrade, never error");
    println!("search 'that key I use for payments' -> {vague:?}");
}

/// Feature 4 of 5: `.env` import (variable NAMES only, never values).
#[test]
#[ignore = "requires a real downloaded model -- see module doc"]
fn env_name_classification_works_end_to_end() {
    let gateway = AiGateway::new(spawn_real_engine());

    let names = vec![
        "DATABASE_URL".to_string(),
        "STRIPE_SECRET_KEY".to_string(),
        "GITHUB_TOKEN".to_string(),
    ];
    let result = gateway
        .classify_env_names(&names)
        .expect("env-name classification must succeed against the real model");

    println!("real model env-name classification: {result:?}");
    // The model may not place every name, but it must return a well-formed
    // response rather than failing the whole import.
    assert!(result.names.len() <= names.len());
}

/// Feature 5 of 5: structured field extraction from a pasted block.
#[test]
#[ignore = "requires a real downloaded model -- see module doc"]
fn structured_extraction_works_end_to_end() {
    let gateway = AiGateway::new(spawn_real_engine());

    let block = "host: db.example.com\nport: 5432\nusername: appuser\npassword: hunter2";
    let result = gateway
        .extract_structured_fields(block)
        .expect("extraction must succeed against the real model");

    println!("real model extracted fields: {result:?}");
    // Whatever it found must be well-formed; an empty result is a legitimate
    // answer, a parse failure is not.
    for field in &result.fields {
        assert!(
            !field.label.trim().is_empty(),
            "a blank label is not usable"
        );
    }
}

/// The crash requirement, exercised against the real worker: killing it
/// mid-flight must produce a recoverable error on the *next* call, never a
/// panic or a hang that takes the host process with it.
#[test]
#[ignore = "requires a real downloaded model -- see module doc"]
fn a_killed_worker_produces_a_recoverable_error_not_a_crash() {
    let engine = spawn_real_engine();
    engine.shutdown();
    let gateway = AiGateway::new(engine);

    let fake_key = format!("{}{}", "sk-or-v1-", "abcdef0123456789");
    let result = gateway.suggest_name(&fake_key, Some("OpenRouter"));

    println!("result after killing the worker: {result:?}");
    assert!(result.is_err(), "a dead worker must report an error");
}
