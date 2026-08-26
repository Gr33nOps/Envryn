//! Test-only fixture standing in for `envryn-ai-worker`'s wire protocol,
//! without pulling in candle/tokenizers or requiring a real model file.
//! Used exclusively by `src/ai/worker_client.rs`'s tests to exercise real
//! process spawning and real loopback socket I/O against a real child
//! process -- the thing that's actually risky to get right in
//! `worker_client.rs` is the framing, the READY-line handshake, and the
//! auth-token check, none of which need real inference to test for real.
//!
//! A throwaway test tool, not part of the vault: panicking on unexpected
//! I/O failure here is fine and expected.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

const TOKEN: &str = "fixture-token";

fn main() {
    if std::env::var("FAKE_WORKER_FAIL_TO_START").is_ok() {
        eprintln!("fake worker: simulated startup failure");
        std::process::exit(1);
    }

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
    let port = listener.local_addr().expect("local_addr").port();
    println!("READY {port} {TOKEN}");
    std::io::stdout().flush().expect("flush stdout");

    for incoming in listener.incoming() {
        let Ok(stream) = incoming else { continue };
        handle_connection(stream);
    }
}

fn handle_connection(mut stream: TcpStream) {
    loop {
        let mut len_bytes = [0u8; 4];
        if stream.read_exact(&mut len_bytes).is_err() {
            break;
        }
        let len = u32::from_le_bytes(len_bytes) as usize;
        let mut buf = vec![0u8; len];
        if stream.read_exact(&mut buf).is_err() {
            break;
        }
        let request: serde_json::Value = serde_json::from_slice(&buf).expect("valid request JSON");

        simulate_inference_delay();

        let response = build_response(&request);

        let bytes = serde_json::to_vec(&response).expect("serialise response");
        let out_len = u32::try_from(bytes.len())
            .expect("response fits in u32")
            .to_le_bytes();
        if stream.write_all(&out_len).is_err() {
            break;
        }
        if stream.write_all(&bytes).is_err() {
            break;
        }
    }
}

/// Simulates "the worker is in the middle of inference" for
/// worker_client's kill-mid-inference test -- a real model taking real
/// wall-clock time to generate is exactly the window a kill needs to land
/// inside to prove anything.
fn simulate_inference_delay() {
    if let Ok(ms) = std::env::var("FAKE_WORKER_DELAY_MS") {
        if let Ok(ms) = ms.parse::<u64>() {
            std::thread::sleep(std::time::Duration::from_millis(ms));
        }
    }
}

/// Lets worker_client's log-sentinel test simulate a worker whose error
/// message happens to contain prompt-shaped content -- this fixture only
/// ever puts it on the wire (below), never on its own stdout/stderr, which
/// is exactly the property that test checks for the real code path too.
fn build_response(request: &serde_json::Value) -> serde_json::Value {
    if let Ok(msg) = std::env::var("FAKE_WORKER_ERROR_MESSAGE") {
        return serde_json::json!({"status": "error", "message": msg});
    }
    if request.get("token").and_then(|t| t.as_str()) == Some(TOKEN) {
        let prompt = request.get("prompt").and_then(|p| p.as_str()).unwrap_or("");
        serde_json::json!({"status": "ok", "text": format!("echo:{prompt}")})
    } else {
        serde_json::json!({"status": "error", "message": "unauthorized"})
    }
}
