//! AI IPC: model download, starting/stopping the local inference worker,
//! and the five Tier-1 features (`docs/AI_DATA_ACCESS.md`).
//!
//! **AI-INV-009, enforced here too, not just in the crate boundary.** Every
//! command in this file is additive -- none of `ipc.rs`'s or `sync.rs`'s
//! commands call into this module, and nothing here is required for a
//! vault to be created, unlocked, edited, or synced. Deleting this file and
//! its registration in `lib.rs` would leave a fully functional vault.
//!
//! **AI is opt-in and fails closed.** Every command below refuses outright
//! if `settings::AppSettings::ai_enabled` is `false`, even if a caller
//! somehow still holds a running engine -- turning the setting off is
//! meant to be a real "no" a stale frontend state cannot route around.
//!
//! **Lock kills the worker.** `ipc::vault_lock` calls [`stop`] before it
//! returns, synchronously -- see `envryn_core::ai::worker_client`'s own doc
//! comment for why killing the process, not asking it to clear its
//! context, is the only thing this crate trusts.
//!
//! **One exception to the "AI must be enabled" rule:** [`classify_deterministic`]
//! is not AI at all -- known-prefix/shape matching in plain Rust
//! (`envryn_core::ai::classify`) that works with the model never installed.
//! It is not gated by `ai_enabled` for that reason; gating it would make
//! the create-secret form's most reliable classification path depend on a
//! setting that has nothing to do with it.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use envryn_core::ai::classify;
use envryn_core::ai::gateway::{AiError, AiGateway};
use envryn_core::ai::model_download::{self, DownloadProgress, ModelFiles, QWEN2_5_1_5B_INSTRUCT};
use envryn_core::ai::schemas::{
    ClassificationOutput, EnvNameClassificationOutput, ExtractedFieldsOutput, NameSuggestionOutput,
    SearchFilterOutput,
};
use envryn_core::ai::worker_client::{self, WorkerClient, WorkerSpawnConfig};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::ipc::{internal, invalid, IpcError, IpcResult};
use crate::settings;

#[tauri::command]
pub fn classify_deterministic(value: String, name: Option<String>) -> Option<ClassificationOutput> {
    if let Some(result) = classify::classify(&value) {
        return Some(ClassificationOutput {
            kind: result.kind,
            provider: result.provider.map(str::to_string),
            confidence: 1.0,
        });
    }
    name.as_deref()
        .and_then(classify::classify_name)
        .map(|(kind, provider)| ClassificationOutput {
            kind,
            provider: (!provider.is_empty()).then_some(provider),
            confidence: 0.9,
        })
}

impl From<AiError> for IpcError {
    fn from(err: AiError) -> Self {
        let message = match &err {
            AiError::BudgetExceeded => {
                "That request is larger than Envryn allows to send to the local model."
            }
            AiError::EngineUnavailable => "The local AI model is not available right now.",
            AiError::EngineTimeout => "The local AI model took too long to respond.",
            AiError::InvalidResponse => "The local AI model could not complete this locally.",
        };
        IpcError {
            code: "ai_unavailable",
            message: message.to_string(),
        }
    }
}

fn models_dir(app: &AppHandle) -> IpcResult<PathBuf> {
    // Deliberately *not* under the vault's own storage directory --
    // docs/AI_SECURITY.md section 7: "Model files are stored under
    // `/models`, never under `/vault`."
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|_| internal("could not locate the application data directory"))?
        .join("models");
    std::fs::create_dir_all(&dir).map_err(|_| internal("could not create the models directory"))?;
    Ok(dir)
}

/// Resolve the worker binary. Tries the Tauri sidecar convention first
/// (bundled alongside the app resources, target-triple-suffixed), then falls
/// back to a plain sibling of the currently running executable.
///
/// Both are real now: `tauri.conf.json`'s `bundle.externalBin` plus
/// `.dev-tools/prepare-sidecar.mjs` (wired into `beforeBuildCommand`) build
/// `envryn-ai-worker` in release mode and place it at
/// `src-tauri/binaries/envryn-ai-worker-<host-triple>.exe`, the exact name
/// `tauri-build`'s own `copy_binaries` step (run from `build.rs`, so it fires
/// on *any* `cargo build`, not only `cargo tauri build`) looks for and copies
/// next to the compiled `envryn` binary -- confirmed by actually running
/// `cargo build -p envryn --release` after placing the sidecar and finding
/// `envryn-ai-worker.exe` alongside `envryn.exe` in `target/release/`
/// unprompted. Packaging into an installed MSI/NSIS bundle specifically
/// (where `resource_dir()` resolves somewhere else entirely) has not been
/// exercised end to end.
fn worker_binary_path(app: &AppHandle) -> IpcResult<PathBuf> {
    if let Ok(resource_dir) = app.path().resource_dir() {
        let candidate = worker_client::worker_binary_path(&resource_dir);
        if candidate.exists() {
            return Ok(candidate);
        }
    }
    let current_exe =
        std::env::current_exe().map_err(|_| internal("could not locate the running executable"))?;
    let dir = current_exe
        .parent()
        .ok_or_else(|| internal("could not locate the running executable's directory"))?;
    let candidate = worker_client::worker_binary_path(dir);
    if candidate.exists() {
        Ok(candidate)
    } else {
        Err(internal(
            "the local AI worker is not installed with this build",
        ))
    }
}

/// The running gateway, if any.
///
/// **The `Arc` is load-bearing, not incidental.** This used to be a plain
/// `Mutex<Option<AiGateway<_>>>` whose lock was held for the *entire*
/// duration of an inference call -- and `ai_status`, a synchronous
/// `#[tauri::command]` that runs directly on Tauri's IPC event loop, locks
/// the same mutex. So any status poll issued while a generation was in
/// flight blocked the IPC thread for as long as the model took, and the
/// whole webview stopped responding: no repaint, no keystroke echo, a real
/// "Envryn (Not Responding)". Cloning an `Arc` out from under a
/// momentarily-held lock and running inference on the clone means the lock
/// is held for a pointer copy instead of for tens of seconds.
///
/// `in_flight` is the second half: without it, N queued requests would each
/// take a turn on the worker's single connection, so a user who clicked
/// twice waited twice as long for an answer they asked for once.
#[derive(Default)]
pub struct AiState {
    gateway: Mutex<Option<Arc<AiGateway<WorkerClient>>>>,
    in_flight: AtomicBool,
}

impl AiState {
    /// Take a reference to the gateway without holding the lock across the
    /// call. Returns the same "not running" error as before when AI is off.
    fn gateway(&self) -> IpcResult<Arc<AiGateway<WorkerClient>>> {
        let guard = self
            .gateway
            .lock()
            .map_err(|_| internal("AI state unavailable"))?;
        guard
            .as_ref()
            .cloned()
            .ok_or_else(|| invalid("Local AI is not running. Enable it in Settings first."))
    }

    /// True when a gateway is present, answered without ever blocking on an
    /// in-progress inference -- `try_lock`, not `lock`, because this is
    /// called from the IPC event loop and a definite "busy" answer is better
    /// than a stalled one. A held lock means a request is being handed off
    /// right now, which only happens while a gateway exists.
    fn is_running(&self) -> bool {
        match self.gateway.try_lock() {
            Ok(guard) => guard.is_some(),
            Err(std::sync::TryLockError::WouldBlock) => true,
            Err(std::sync::TryLockError::Poisoned(_)) => false,
        }
    }

    /// Run one AI operation, refusing to start a second while one is still
    /// running. The worker serves a single connection serially, so a
    /// concurrent second request would not be faster -- it would queue
    /// behind the first while the user watched two spinners.
    fn with<T>(
        &self,
        f: impl FnOnce(&AiGateway<WorkerClient>) -> Result<T, AiError>,
    ) -> IpcResult<T> {
        let gateway = self.gateway()?;
        if self.in_flight.swap(true, Ordering::SeqCst) {
            return Err(IpcError {
                code: "ai_busy",
                message: "The local AI model is already working on another request.".to_string(),
            });
        }
        let _guard = InFlightGuard(&self.in_flight);
        Ok(f(&gateway)?)
    }
}

/// Clears the in-flight flag however `with` exits -- including on an early
/// `?` return or a panic inside the closure. Without this, one failed
/// request would leave AI permanently reporting itself as busy.
struct InFlightGuard<'a>(&'a AtomicBool);

impl Drop for InFlightGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

fn require_enabled(app: &AppHandle) -> IpcResult<()> {
    #[cfg(target_os = "android")]
    {
        let _ = app;
        return Err(invalid("Local AI is not available on Android."));
    }
    #[cfg(not(target_os = "android"))]
    {
        if settings::load(app).ai_enabled {
            Ok(())
        } else {
            Err(invalid("Local AI is turned off. Enable it in Settings."))
        }
    }
}

#[derive(Serialize, ts_rs::TS)]
#[ts(export)]
pub struct AiStatus {
    pub enabled_in_settings: bool,
    pub model_downloaded: bool,
    pub model_name: &'static str,
    pub engine_running: bool,
}

#[tauri::command]
pub fn ai_status(app: AppHandle, state: State<'_, AiState>) -> IpcResult<AiStatus> {
    let dir = models_dir(&app)?;
    let model_downloaded = model_download::already_verified(&QWEN2_5_1_5B_INSTRUCT, &dir).is_some();
    let engine_running = state.is_running();
    Ok(AiStatus {
        enabled_in_settings: settings::load(&app).ai_enabled,
        model_downloaded,
        model_name: QWEN2_5_1_5B_INSTRUCT.display_name,
        engine_running,
    })
}

/// Emitted repeatedly to `"ai://download-progress"` while [`ai_download_model`]
/// runs, so the settings screen can show real progress instead of an
/// indeterminate spinner for what is, on an ordinary connection, a
/// multi-minute wait for the ~350&nbsp;MB model file.
#[derive(Clone, Serialize, ts_rs::TS)]
#[ts(export, rename = "AiDownloadProgress")]
struct AiDownloadProgressEvent {
    file_name: String,
    bytes_downloaded: u64,
    total_bytes: u64,
}

impl From<DownloadProgress> for AiDownloadProgressEvent {
    fn from(p: DownloadProgress) -> Self {
        Self {
            file_name: p.file_name.to_string(),
            bytes_downloaded: p.bytes_downloaded,
            total_bytes: p.total_bytes,
        }
    }
}

/// Download (or confirm already-downloaded) the one pinned Tier-1 model.
/// Blocking and potentially slow (a few hundred megabytes) -- run behind
/// `spawn_blocking` so it does not stall the IPC event loop.
#[tauri::command]
pub async fn ai_download_model(app: AppHandle) -> IpcResult<()> {
    let dir = models_dir(&app)?;
    let progress_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        model_download::download_and_verify_with_progress(&QWEN2_5_1_5B_INSTRUCT, &dir, &mut |p| {
            let _ = progress_app.emit("ai://download-progress", AiDownloadProgressEvent::from(p));
        })
        .map(|_: ModelFiles| ())
        .map_err(|_| internal("Could not download or verify the local AI model."))
    })
    .await
    .map_err(|_| internal("model download task failed"))?
}

/// Start the local AI worker. Requires the model to already be downloaded
/// and verified -- this command never triggers a download itself, keeping
/// "fetch something over the network" and "run the local model" as two
/// separate, separately-confirmed actions.
#[tauri::command]
pub async fn ai_start(app: AppHandle, state: State<'_, AiState>) -> IpcResult<()> {
    require_enabled(&app)?;
    let dir = models_dir(&app)?;
    let files = model_download::already_verified(&QWEN2_5_1_5B_INSTRUCT, &dir)
        .ok_or_else(|| invalid("Download the local AI model in Settings first."))?;
    let worker_binary = worker_binary_path(&app)?;

    {
        let guard = state
            .gateway
            .lock()
            .map_err(|_| internal("AI state unavailable"))?;
        if guard.is_some() {
            return Ok(());
        }
    }

    let config = WorkerSpawnConfig {
        worker_binary,
        model_path: files.model_path,
        tokenizer_path: files.tokenizer_path,
        arch: QWEN2_5_1_5B_INSTRUCT.arch.to_string(),
        eos_token: QWEN2_5_1_5B_INSTRUCT.eos_token.to_string(),
        extra_env: Vec::new(),
    };

    let gateway = tauri::async_runtime::spawn_blocking(move || {
        WorkerClient::spawn(&config).map(AiGateway::new)
    })
    .await
    .map_err(|_| internal("AI worker startup task failed"))?
    .map_err(|_| internal("Could not start the local AI model."))?;

    let mut guard = state
        .gateway
        .lock()
        .map_err(|_| internal("AI state unavailable"))?;
    *guard = Some(Arc::new(gateway));
    Ok(())
}

/// Stop the worker, if running. Infallible from the caller's point of view,
/// matching `ipc::vault_lock` -- a lock (or an explicit "turn AI off") must
/// never fail to actually stop the process.
#[tauri::command]
pub fn ai_stop(state: State<'_, AiState>) {
    stop(&state);
}

pub fn stop(state: &AiState) {
    if let Ok(mut guard) = state.gateway.lock() {
        // Dropping the last Arc runs AiGateway -> WorkerClient::drop(),
        // which kills the process. An in-flight request holds a clone, so
        // the kill lands when that request finishes rather than tearing the
        // socket out from under it -- and `WorkerClient`'s Job Object still
        // guarantees termination even if this process dies before then.
        *guard = None;
    }
}

// Every command below is `async fn` + `spawn_blocking`, matching
// `ai_download_model`/`ai_start` -- each does real blocking socket I/O to
// the worker subprocess to run inference (`AiGateway::run`, synchronous by
// construction). A plain sync `#[tauri::command]` runs on Tauri's IPC event
// loop, so a slow inference call there stalls every other pending IPC
// message -- the whole webview looks hung (no repaint, no keystroke echo)
// until it returns. `State<'_, AiState>` cannot be moved into a `'static`
// blocking closure, so these re-fetch it from the owned `AppHandle` instead
// (`app.state::<AiState>()`), which is equivalent -- the state lives for
// the app's lifetime regardless of which handle borrows it.

/// **A deterministic match is final; the model never gets to overrule it.**
///
/// The frontend already tries `classify_deterministic` first, but that was a
/// convention a caller could forget -- and one did, which is how an
/// OpenRouter key (a literal, unambiguous `sk-or-v1-` prefix) ended up
/// labelled "Stripe" by a 1.5B model guessing at a string it had no reason
/// to recognise. Short-circuiting here makes the precedence a property of
/// the command itself: for any value the rules recognise, this returns the
/// rules' answer at full confidence without the worker being consulted at
/// all -- faster, and correct even if the model is running and confident.
#[tauri::command]
pub async fn ai_classify_pasted_value(
    app: AppHandle,
    value: String,
) -> IpcResult<ClassificationOutput> {
    if let Some(deterministic) = classify::classify(&value) {
        return Ok(ClassificationOutput {
            kind: deterministic.kind,
            provider: deterministic.provider.map(str::to_string),
            confidence: 1.0,
        });
    }
    require_enabled(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<AiState>()
            .with(|g| g.classify_pasted_value(&value))
    })
    .await
    .map_err(|_| internal("AI task failed"))?
}

#[tauri::command]
pub async fn ai_suggest_name(
    app: AppHandle,
    value: String,
    provider: Option<String>,
) -> IpcResult<NameSuggestionOutput> {
    require_enabled(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<AiState>()
            .with(|g| g.suggest_name(&value, provider.as_deref()))
    })
    .await
    .map_err(|_| internal("AI task failed"))?
}

#[tauri::command]
pub async fn ai_classify_env_names(
    app: AppHandle,
    names: Vec<String>,
) -> IpcResult<EnvNameClassificationOutput> {
    require_enabled(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<AiState>()
            .with(|g| g.classify_env_names(&names))
    })
    .await
    .map_err(|_| internal("AI task failed"))?
}

#[tauri::command]
pub async fn ai_extract_structured_fields(
    app: AppHandle,
    block: String,
) -> IpcResult<ExtractedFieldsOutput> {
    require_enabled(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<AiState>()
            .with(|g| g.extract_structured_fields(&block))
    })
    .await
    .map_err(|_| internal("AI task failed"))?
}

/// **Search never fails closed.** Unlike every other command here, this one
/// does not refuse when AI is off, not downloaded, or crashed -- it falls
/// back to `envryn_core::ai::search::parse_query`, which needs no model at
/// all. Search is the one AI-adjacent feature a user reaches for constantly,
/// and "the local model isn't running" is not a useful answer to "find my
/// production Stripe key" when the vault can answer that from metadata.
#[tauri::command]
pub async fn ai_parse_search_intent(
    app: AppHandle,
    query: String,
) -> IpcResult<SearchFilterOutput> {
    let deterministic = envryn_core::ai::search::parse_query(&query);
    if require_enabled(&app).is_err() {
        return Ok(deterministic);
    }
    let fallback = deterministic.clone();
    let parsed = tauri::async_runtime::spawn_blocking(move || {
        app.state::<AiState>()
            .with(|g| g.parse_search_intent(&query))
    })
    .await
    .map_err(|_| internal("AI task failed"))?;
    // A worker that is down, busy, or returned nonsense degrades to the
    // deterministic parse instead of surfacing an error the user can do
    // nothing about mid-search.
    Ok(parsed.unwrap_or(fallback))
}
