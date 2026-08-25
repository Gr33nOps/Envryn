//! A [`LocalAiEngine`] that talks to `crates/envryn-ai-worker` over a
//! loopback TCP connection.
//!
//! **This module owns process spawning too**, not just the client side of
//! the wire protocol -- deliberately, so it can be tested as a plain
//! library the same way `crate::sync`'s transport code is (real process,
//! real socket, no mock), rather than pushing that logic into `src-tauri`
//! where it could only be exercised by clicking through the app. The Tauri
//! shell's job is limited to resolving *where the worker binary lives on
//! disk* (a Tauri-specific concern -- sidecar bundling) and choosing *which
//! already-downloaded, checksum-verified model file to load* -- both of
//! which are handed in here as plain paths.
//!
//! Framing matches `crate::sync::protocol`'s length-prefixed JSON exactly,
//! reusing its `write_json`/`read_json` rather than a third implementation
//! of the same idea.

use std::io::{BufRead, BufReader};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::ai::engine::{EngineError, LocalAiEngine, SchemaKind};
use crate::ai::gateway::SanitizedPrompt;
use crate::sync::protocol::{read_json, write_json};

const READY_TIMEOUT: Duration = Duration::from_secs(60);
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(120);

/// The wire spelling for each constrainable [`SchemaKind`]. A plain string,
/// not a shared enum type: this crate and `envryn-ai-worker` deliberately
/// do not share a protocol crate (`envryn-ai-worker` must not depend on
/// `envryn-core` at all, AI-INV-001/002/004/005), so the wire format itself
/// is the only contract between them. `None` (the field omitted) means
/// `SchemaKind::Unconstrained` -- ordinary prompting only, matching every
/// worker build before this field existed.
fn schema_wire_name(schema: SchemaKind) -> Option<&'static str> {
    match schema {
        SchemaKind::Unconstrained => None,
        SchemaKind::ClassificationOutput => Some("classification_output"),
    }
}

#[derive(Serialize)]
struct WireRequest {
    token: String,
    prompt: String,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    schema: Option<&'static str>,
}

#[derive(Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum WireResponse {
    Ok {
        text: String,
    },
    // The field must stay named `message` to deserialise the worker's wire
    // shape (`{"status":"error","message":"..."}`) at all -- but `complete`
    // deliberately never reads it (bound to `_` at its one match site) and
    // never will, so `#[allow(dead_code)]` here is exact, not a blanket
    // suppression: see that match arm's comment for why.
    #[allow(dead_code)]
    Error {
        message: String,
    },
}

/// Where to find the worker binary and the already-verified model it should
/// load. Every field here is a plain path or string the caller already
/// resolved -- this type carries no opinion about *how* they were chosen.
pub struct WorkerSpawnConfig {
    pub worker_binary: PathBuf,
    pub model_path: PathBuf,
    pub tokenizer_path: PathBuf,
    pub arch: String,
    pub eos_token: String,
    /// Extra environment variables for the child process. Empty in
    /// production; used by this module's own tests to make the test
    /// fixture binary simulate a failed model load.
    pub extra_env: Vec<(String, String)>,
}

/// A running worker process and the connection to it. Killing the process
/// on drop is the safety net; callers that need the kill to happen
/// *synchronously* (e.g. right before vault lock returns) should call
/// [`WorkerClient::shutdown`] explicitly rather than relying on drop order.
///
/// `_job` is a second, independent safety net for the case the first one
/// (this struct's own `Drop`) cannot cover: if the whole Envryn process
/// exits abnormally (crash, a forceful kill) without running any `Drop` at
/// all, `crate::platform::KillOnCloseJob`'s own `Drop` -- which the OS runs
/// as part of closing every handle this process still held, even on an
/// abnormal exit -- terminates the worker anyway. Never read after
/// construction; it exists to be dropped at the right time, not used.
pub struct WorkerClient {
    child: Mutex<Child>,
    stream: Mutex<TcpStream>,
    token: String,
    _job: Option<crate::platform::KillOnCloseJob>,
}

impl WorkerClient {
    /// Spawn the worker and block until it reports readiness or
    /// [`READY_TIMEOUT`] elapses. The worker prints its bound port and a
    /// bearer token as its first stdout line (`READY <port> <token>`) --
    /// see `crates/envryn-ai-worker/src/main.rs`.
    pub fn spawn(config: &WorkerSpawnConfig) -> Result<Self, EngineError> {
        let mut command = Command::new(&config.worker_binary);
        command
            .arg("--model")
            .arg(&config.model_path)
            .arg("--tokenizer")
            .arg(&config.tokenizer_path)
            .arg("--arch")
            .arg(&config.arch)
            .arg("--eos-token")
            .arg(&config.eos_token)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (key, value) in &config.extra_env {
            command.env(key, value);
        }
        let mut child = command.spawn().map_err(|_| EngineError::Unavailable)?;

        // Best-effort: a platform without `KillOnCloseJob` (anything but
        // Windows today, see `platform::stub`) or a process that could not
        // be assigned (already in a non-nesting job -- rare, but Windows
        // permits it) still has the ordinary `Drop`-based kill; this is
        // additive hardening, not the only thing keeping the worker
        // supervised, so a failure here is not fatal to spawning at all.
        let job = crate::platform::KillOnCloseJob::new().ok();
        if let Some(job) = &job {
            #[cfg(windows)]
            {
                use std::os::windows::io::AsRawHandle;
                let _ = job.assign(child.as_raw_handle() as isize);
            }
            #[cfg(not(windows))]
            {
                let _ = job;
            }
        }

        let stdout = child.stdout.take().ok_or(EngineError::Unavailable)?;
        let (port, token) = match read_ready_line_with_timeout(stdout, READY_TIMEOUT) {
            Ok(ready) => ready,
            Err(e) => {
                let _ = child.kill();
                return Err(e);
            }
        };

        let stream = TcpStream::connect(("127.0.0.1", port)).map_err(|_| {
            let _ = child.kill();
            EngineError::Unavailable
        })?;
        stream
            .set_read_timeout(Some(RESPONSE_TIMEOUT))
            .map_err(|_| EngineError::Unavailable)?;

        Ok(Self {
            child: Mutex::new(child),
            stream: Mutex::new(stream),
            token,
            _job: job,
        })
    }

    /// Kill the worker process immediately. `docs/AI_SECURITY.md` section 3:
    /// killing the process, not clearing its context, is the only thing
    /// that can be trusted to remove whatever plaintext was in its
    /// inference buffers -- called on vault lock, not just on drop, so the
    /// termination is synchronous with the lock operation returning.
    pub fn shutdown(&self) {
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Drop for WorkerClient {
    fn drop(&mut self) {
        self.shutdown();
    }
}

impl LocalAiEngine for WorkerClient {
    fn complete(&self, prompt: &SanitizedPrompt, max_tokens: u32) -> Result<String, EngineError> {
        self.complete_for_schema(prompt, max_tokens, SchemaKind::Unconstrained)
    }

    fn complete_for_schema(
        &self,
        prompt: &SanitizedPrompt,
        max_tokens: u32,
        schema: SchemaKind,
    ) -> Result<String, EngineError> {
        let mut stream = self.stream.lock().map_err(|_| EngineError::Unavailable)?;
        let request = WireRequest {
            token: self.token.clone(),
            prompt: prompt.expose().to_string(),
            max_tokens,
            schema: schema_wire_name(schema),
        };
        write_json(&mut *stream, &request).map_err(|_| EngineError::Unavailable)?;
        let response: WireResponse = read_json(&mut *stream).map_err(|_| EngineError::Timeout)?;
        match response {
            WireResponse::Ok { text } => Ok(text),
            // `message` is meant to be the worker's own diagnostic about its
            // own operational failure (e.g. "tokenizer encode failed"), but
            // that is a convention the worker's error paths follow, not
            // something this client can verify -- a future bug in a
            // library error's Display impl, or in how the worker builds
            // this string, could put a fragment of the prompt or model
            // output into it. Rather than trust that and print it
            // (docs/AI_SECURITY.md section 6: never the prompt, the model
            // input, the model output, or any fragment of a value), the
            // message is discarded entirely and only the fact of failure is
            // observable, matching the "operation name, status, and timing
            // only" rule exactly rather than relying on convention to keep
            // it that way.
            WireResponse::Error { message: _ } => Err(EngineError::Malformed),
        }
    }
}

/// Reads the worker's `READY <port> <token>` line on a background thread so
/// a worker that hangs before printing it (stuck loading a corrupt or
/// oversized model, a bug) cannot block `spawn` forever -- `recv_timeout`
/// bounds the wait even though `Read::read_line` itself has no timeout.
fn read_ready_line_with_timeout(
    stdout: impl std::io::Read + Send + 'static,
    timeout: Duration,
) -> Result<(u16, String), EngineError> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(read_ready_line(stdout));
    });
    rx.recv_timeout(timeout).map_err(|_| EngineError::Timeout)?
}

fn read_ready_line(stdout: impl std::io::Read) -> Result<(u16, String), EngineError> {
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    // A worker that fails to load its model exits before printing anything,
    // so `read_line` returning Ok(0) (EOF) is the expected shape of "the
    // model would not load" -- not distinguished further here; the caller
    // already gets `EngineError::Unavailable` either way, matching
    // AI-INV-009 (a failed model load is not treated differently from "no
    // AI installed at all").
    reader
        .read_line(&mut line)
        .map_err(|_| EngineError::Unavailable)?;
    let mut parts = line.split_whitespace();
    if parts.next() != Some("READY") {
        return Err(EngineError::Unavailable);
    }
    let port: u16 = parts
        .next()
        .and_then(|p| p.parse().ok())
        .ok_or(EngineError::Unavailable)?;
    let token = parts.next().ok_or(EngineError::Unavailable)?.to_string();
    Ok((port, token))
}

/// Resolve the worker binary's expected filename for this platform, given
/// its containing directory. Windows binaries carry `.exe`; nothing else
/// about the name is platform-specific.
pub fn worker_binary_path(dir: &Path) -> PathBuf {
    let name = if cfg!(windows) {
        "envryn-ai-worker.exe"
    } else {
        "envryn-ai-worker"
    };
    dir.join(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `tests/fixtures/fake_worker_main.rs` speaks the exact same
    /// READY-line + length-prefixed-JSON protocol the real worker does,
    /// without needing candle, a tokenizer, or a multi-hundred-megabyte
    /// model file -- so this exercises real process spawning, real
    /// loopback socket I/O, and the real framing in `write_json`/
    /// `read_json`, which is what is actually risky to get right in this
    /// module. It does not prove the real model produces good output --
    /// that has no security property to test here, only a quality one.
    /// `CARGO_BIN_EXE_*` is only populated for integration tests (files
    /// under `tests/`), not for a lib's own unit tests -- and these tests
    /// need `pub(crate)` access (`SanitizedPrompt::for_test`), so they stay
    /// unit tests rather than moving to `tests/`. Every `[[bin]]` target,
    /// including the fixture, lands next to the currently-running test
    /// binary's `deps/` directory, which is a stable enough Cargo layout to
    /// rely on here.
    fn fixture_binary_path() -> PathBuf {
        let exe_ext = std::env::consts::EXE_SUFFIX;
        let mut dir = std::env::current_exe().unwrap();
        dir.pop(); // this test binary's file name
        if dir.ends_with("deps") {
            dir.pop();
        }
        dir.join(format!("fake-ai-worker-fixture{exe_ext}"))
    }

    fn fixture_config() -> WorkerSpawnConfig {
        WorkerSpawnConfig {
            worker_binary: fixture_binary_path(),
            model_path: PathBuf::from("unused-by-the-fixture.gguf"),
            tokenizer_path: PathBuf::from("unused-by-the-fixture.json"),
            arch: "qwen2".to_string(),
            eos_token: "<|im_end|>".to_string(),
            extra_env: Vec::new(),
        }
    }

    #[test]
    fn spawn_connects_and_a_real_request_round_trips() {
        let client = WorkerClient::spawn(&fixture_config()).unwrap();
        let prompt = SanitizedPrompt::for_test("hello worker");
        let text = client.complete(&prompt, 16).unwrap();
        assert_eq!(text, "echo:hello worker");
        client.shutdown();
    }

    /// Mirrors the real worker's actual behaviour when `Engine::load` fails
    /// (`crates/envryn-ai-worker/src/main.rs`: print to stderr, exit before
    /// printing READY) -- the fixture takes the identical path when this
    /// env var is set, so this proves `WorkerClient::spawn` handles "the
    /// worker started but could not load its model" the same way it
    /// handles "the worker binary itself doesn't exist": cleanly, as
    /// `EngineError::Unavailable`, never a panic or a hang waiting on a
    /// READY line that will never arrive.
    #[test]
    fn spawn_reports_unavailable_when_the_worker_fails_to_load_its_model() {
        let mut config = fixture_config();
        config
            .extra_env
            .push(("FAKE_WORKER_FAIL_TO_START".to_string(), "1".to_string()));
        let result = WorkerClient::spawn(&config);
        assert!(matches!(result, Err(EngineError::Unavailable)));
    }

    #[test]
    fn spawn_reports_unavailable_when_the_worker_binary_does_not_exist() {
        let mut config = fixture_config();
        config.worker_binary = PathBuf::from("this-binary-does-not-exist");
        let result = WorkerClient::spawn(&config);
        assert!(matches!(result, Err(EngineError::Unavailable)));
    }

    #[test]
    fn shutdown_actually_terminates_the_child_process() {
        let client = WorkerClient::spawn(&fixture_config()).unwrap();
        client.shutdown();
        // A killed child can be waited on without blocking -- try_wait
        // returning Some(_) proves the process has actually exited, not
        // merely that `kill()` was called without error.
        let mut child = client.child.lock().unwrap();
        std::thread::sleep(Duration::from_millis(200));
        assert!(child.try_wait().unwrap().is_some());
    }

    /// The adversarial scenario `docs/AI_SECURITY.md` names explicitly:
    /// "kill the worker mid-inference and confirm the vault is unaffected."
    /// The fixture is told to sleep before answering (standing in for real
    /// generation time); a second thread kills the process while that sleep
    /// is in progress, i.e. genuinely mid-request, not merely mid-idle like
    /// `shutdown_actually_terminates_the_child_process` above. `complete`
    /// must return an error, not hang forever waiting on a response that
    /// will now never arrive, and not panic. "The vault is unaffected" is
    /// not a separate assertion here -- it is structural: nothing in this
    /// crate's `vault`/`storage`/`crypto` modules holds a reference to, or
    /// blocks on, an `AiGateway` or `WorkerClient` at all, so there is no
    /// vault-side state this scenario could even reach into.
    #[test]
    fn killing_the_worker_mid_inference_fails_the_request_cleanly() {
        let mut config = fixture_config();
        config
            .extra_env
            .push(("FAKE_WORKER_DELAY_MS".to_string(), "5000".to_string()));
        let client = WorkerClient::spawn(&config).unwrap();
        let client = std::sync::Arc::new(client);

        let request_client = client.clone();
        let handle = std::thread::spawn(move || {
            let prompt = SanitizedPrompt::for_test("this will be interrupted");
            request_client.complete(&prompt, 16)
        });

        // Give the request time to actually reach the fixture and start its
        // 5-second sleep before killing -- otherwise this could race and
        // kill before the connection is even established.
        std::thread::sleep(Duration::from_millis(300));
        client.shutdown();

        let result = handle
            .join()
            .expect("the request thread itself must not panic");
        assert!(
            matches!(
                result,
                Err(EngineError::Timeout) | Err(EngineError::Unavailable)
            ),
            "expected a clean engine error, got {result:?}"
        );
    }

    /// `docs/AI_SECURITY.md` section 6's sentinel scenario: even if a
    /// worker's error message happened to contain something prompt-shaped
    /// (simulated here via the fixture -- the real worker's own error paths
    /// do not do this today, but nothing *type-level* stopped a future
    /// change from introducing it), that message must never be observable
    /// outside this function. `complete`'s only handling of
    /// `WireResponse::Error` is `Err(EngineError::Malformed)` -- the
    /// `message` field is bound to `_` and never read (see its call site) --
    /// so this asserts the *outcome* carries no trace of the sentinel: not
    /// in the returned error (`EngineError` has no message-carrying variant
    /// at all to put it in) and not anywhere reachable from this test.
    /// A real "capture this process's OS-level stdout/stderr" check would
    /// need unsafe FFI this crate's `unsafe_code = "deny"` lint reserves for
    /// `platform::windows_impl` alone -- not worth adding for a risk this
    /// change already removed structurally; the static guarantee is instead
    /// `.semgrep/ai-no-content-logging.yml`'s job, verified separately.
    #[test]
    fn a_sentinel_in_a_worker_error_message_produces_no_message_carrying_result() {
        const SENTINEL: &str = "sk-test-SENTINEL-VALUE-MUST-NEVER-BE-OBSERVABLE-abc123";

        let mut config = fixture_config();
        config.extra_env.push((
            "FAKE_WORKER_ERROR_MESSAGE".to_string(),
            SENTINEL.to_string(),
        ));
        let client = WorkerClient::spawn(&config).unwrap();

        let prompt = SanitizedPrompt::for_test("irrelevant, the response is forced to error");
        let result = client.complete(&prompt, 8);

        // `EngineError::Malformed` is a unit variant -- there is no `Debug`
        // or `Display` rendering of this `Result` that could contain the
        // sentinel, because there is no field to put it in.
        assert!(matches!(result, Err(EngineError::Malformed)));
        assert!(!format!("{result:?}").contains(SENTINEL));

        client.shutdown();
    }
}
