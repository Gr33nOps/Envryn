//! Envryn's local AI inference process.
//!
//! Spawned by the Tauri shell as a child process (`envryn_core::ai::worker_client`
//! owns the spawn+connect logic; this binary only speaks the protocol once
//! running). Receives a model path, a tokenizer path, and nothing else that
//! identifies the vault -- no database path, no key, and this crate does
//! not depend on `envryn-core` at all, so it *cannot* name a vault type
//! even if a bug tried to hand it one (AI-INV-001/002/004/005, checked by
//! `cargo tree` in this repo's verification, not just asserted here).
//!
//! Protocol: binds `127.0.0.1:0`, prints `READY <port> <token>\n` to stdout
//! and flushes, then accepts connections and answers length-prefixed JSON
//! requests (`protocol::Request`/`protocol::Response`) one at a time.
//! Requests missing the printed token are refused before touching the
//! model. There is deliberately no other command -- shutdown is the parent
//! killing this process outright (`docs/AI_SECURITY.md` section 3: "the
//! only way to be genuinely confident no plaintext survives... is killing
//! the process"), not a message this binary listens for.

mod constrained;
mod model;
mod protocol;

use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;

use constrained::JsonGrammar;
use model::Engine;

/// `SecretKind`'s exact serialised variant names, matched by
/// `crates/envryn-core/src/model.rs::SecretKind`'s plain (no
/// `#[serde(rename_all)]`) derive -- checked against the real enum by
/// `crates/envryn-core/tests/ai_real_model.rs`'s use of this same worker,
/// not just assumed here. Duplicated rather than shared because this crate
/// must not depend on `envryn-core` at all (AI-INV-001/002/004/005).
const SECRET_KIND_VARIANTS: &[&str] = &[
    "ApiKey", "Token", "EnvVar", "Database", "Ssh", "OAuth", "Webhook", "Note", "Custom",
];

struct Args {
    model: PathBuf,
    tokenizer: PathBuf,
    arch: String,
    eos_token: String,
}

fn parse_args() -> Result<Args, String> {
    let mut model = None;
    let mut tokenizer = None;
    let mut arch = "qwen2".to_string();
    let mut eos_token = "<|im_end|>".to_string();

    let mut raw = std::env::args().skip(1);
    while let Some(flag) = raw.next() {
        let value = raw
            .next()
            .ok_or_else(|| format!("{flag} requires a value"))?;
        match flag.as_str() {
            "--model" => model = Some(PathBuf::from(value)),
            "--tokenizer" => tokenizer = Some(PathBuf::from(value)),
            "--arch" => arch = value,
            "--eos-token" => eos_token = value,
            other => return Err(format!("unrecognised argument: {other}")),
        }
    }

    Ok(Args {
        model: model.ok_or("--model is required")?,
        tokenizer: tokenizer.ok_or("--tokenizer is required")?,
        arch,
        eos_token,
    })
}

fn random_token() -> String {
    use rand::Rng;
    let bytes: [u8; 24] = rand::thread_rng().r#gen();
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn handle_connection(stream: &mut TcpStream, engine: &Engine, token: &str) {
    loop {
        let request: protocol::Request = match protocol::read_json(stream) {
            Ok(r) => r,
            Err(_) => return, // connection closed or malformed frame -- nothing more to do
        };

        let response = if request.token != token {
            protocol::Response::Error {
                message: "unauthorized".to_string(),
            }
        } else {
            let result = match request.schema.as_deref() {
                Some("classification_output") => {
                    let grammar = JsonGrammar::classification_output(SECRET_KIND_VARIANTS);
                    engine.generate_constrained(&request.prompt, request.max_tokens, &grammar)
                }
                // An unrecognised schema name degrades to ordinary
                // generation rather than failing the request outright -- a
                // client built against a schema this worker build does not
                // know about should not lose the feature entirely, only the
                // structural guarantee.
                _ => engine.generate(&request.prompt, request.max_tokens),
            };
            match result {
                Ok(text) => protocol::Response::Ok { text },
                Err(message) => protocol::Response::Error { message },
            }
        };

        if protocol::write_json(stream, &response).is_err() {
            return;
        }
    }
}

fn main() {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("envryn-ai-worker: {e}");
            std::process::exit(2);
        }
    };

    let engine = match Engine::load(&args.arch, &args.model, &args.tokenizer, &args.eos_token) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("envryn-ai-worker: failed to load model: {e}");
            std::process::exit(1);
        }
    };

    let listener = match TcpListener::bind("127.0.0.1:0") {
        Ok(l) => l,
        Err(e) => {
            eprintln!("envryn-ai-worker: could not bind a loopback port: {e}");
            std::process::exit(1);
        }
    };
    let port = match listener.local_addr() {
        Ok(a) => a.port(),
        Err(e) => {
            eprintln!("envryn-ai-worker: could not read the bound port: {e}");
            std::process::exit(1);
        }
    };
    let token = random_token();

    println!("READY {port} {token}");
    if std::io::stdout().flush().is_err() {
        std::process::exit(1);
    }

    for incoming in listener.incoming() {
        let Ok(mut stream) = incoming else { continue };
        // One connection at a time, processed inline: inference is
        // CPU-serial regardless, and a sidecar meant to serve exactly one
        // Envryn instance has no reason to accept concurrent callers.
        handle_connection(&mut stream, &engine, &token);
    }
}
