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
use std::sync::Mutex;

use envryn_core::ai::classify::{self, DeterministicMatch};
use envryn_core::ai::gateway::{AiError, AiGateway};
use envryn_core::ai::model_download::{self, ModelFiles, QWEN2_0_5B_INSTRUCT};
use envryn_core::ai::schemas::{
    ClassificationOutput, EnvNameClassificationOutput, ExtractedFieldsOutput, NameSuggestionOutput,
    SearchFilterOutput,
};
use envryn_core::ai::worker_client::{self, WorkerClient, WorkerSpawnConfig};
use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::ipc::{internal, invalid, IpcError, IpcResult};
use crate::settings;

#[tauri::command]
pub fn classify_deterministic(value: String) -> Option<DeterministicMatch> {
    classify::classify(&value)
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

#[derive(Default)]
pub struct AiState(Mutex<Option<AiGateway<WorkerClient>>>);

impl AiState {
    fn with<T>(
        &self,
        f: impl FnOnce(&AiGateway<WorkerClient>) -> Result<T, AiError>,
    ) -> IpcResult<T> {
        let guard = self
            .0
            .lock()
            .map_err(|_| internal("AI state unavailable"))?;
        let gateway = guard
            .as_ref()
            .ok_or_else(|| invalid("Local AI is not running. Enable it in Settings first."))?;
        Ok(f(gateway)?)
    }
}

fn require_enabled(app: &AppHandle) -> IpcResult<()> {
    if settings::load(app).ai_enabled {
        Ok(())
    } else {
        Err(invalid("Local AI is turned off. Enable it in Settings."))
    }
}

#[derive(Serialize)]
pub struct AiStatus {
    pub enabled_in_settings: bool,
    pub model_downloaded: bool,
    pub model_name: &'static str,
    pub engine_running: bool,
}

#[tauri::command]
pub fn ai_status(app: AppHandle, state: State<'_, AiState>) -> IpcResult<AiStatus> {
    let dir = models_dir(&app)?;
    let model_downloaded = model_download::already_verified(&QWEN2_0_5B_INSTRUCT, &dir).is_some();
    let engine_running = state.0.lock().map(|g| g.is_some()).unwrap_or(false);
    Ok(AiStatus {
        enabled_in_settings: settings::load(&app).ai_enabled,
        model_downloaded,
        model_name: QWEN2_0_5B_INSTRUCT.display_name,
        engine_running,
    })
}

/// Download (or confirm already-downloaded) the one pinned Tier-1 model.
/// Blocking and potentially slow (a few hundred megabytes) -- run behind
/// `spawn_blocking` so it does not stall the IPC event loop.
#[tauri::command]
pub async fn ai_download_model(app: AppHandle) -> IpcResult<()> {
    let dir = models_dir(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        model_download::download_and_verify(&QWEN2_0_5B_INSTRUCT, &dir)
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
    let files = model_download::already_verified(&QWEN2_0_5B_INSTRUCT, &dir)
        .ok_or_else(|| invalid("Download the local AI model in Settings first."))?;
    let worker_binary = worker_binary_path(&app)?;

    {
        let guard = state
            .0
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
        arch: QWEN2_0_5B_INSTRUCT.arch.to_string(),
        eos_token: QWEN2_0_5B_INSTRUCT.eos_token.to_string(),
        extra_env: Vec::new(),
    };

    let gateway = tauri::async_runtime::spawn_blocking(move || {
        WorkerClient::spawn(&config).map(AiGateway::new)
    })
    .await
    .map_err(|_| internal("AI worker startup task failed"))?
    .map_err(|_| internal("Could not start the local AI model."))?;

    let mut guard = state
        .0
        .lock()
        .map_err(|_| internal("AI state unavailable"))?;
    *guard = Some(gateway);
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
    if let Ok(mut guard) = state.0.lock() {
        *guard = None; // AiGateway -> WorkerClient::drop() kills the process
    }
}

#[tauri::command]
pub fn ai_classify_pasted_value(
    app: AppHandle,
    state: State<'_, AiState>,
    value: String,
) -> IpcResult<ClassificationOutput> {
    require_enabled(&app)?;
    state.with(|g| g.classify_pasted_value(&value))
}

#[tauri::command]
pub fn ai_suggest_name(
    app: AppHandle,
    state: State<'_, AiState>,
    value: String,
    provider: Option<String>,
) -> IpcResult<NameSuggestionOutput> {
    require_enabled(&app)?;
    state.with(|g| g.suggest_name(&value, provider.as_deref()))
}

#[tauri::command]
pub fn ai_classify_env_names(
    app: AppHandle,
    state: State<'_, AiState>,
    names: Vec<String>,
) -> IpcResult<EnvNameClassificationOutput> {
    require_enabled(&app)?;
    state.with(|g| g.classify_env_names(&names))
}

#[tauri::command]
pub fn ai_extract_structured_fields(
    app: AppHandle,
    state: State<'_, AiState>,
    block: String,
) -> IpcResult<ExtractedFieldsOutput> {
    require_enabled(&app)?;
    state.with(|g| g.extract_structured_fields(&block))
}

#[tauri::command]
pub fn ai_parse_search_intent(
    app: AppHandle,
    state: State<'_, AiState>,
    query: String,
) -> IpcResult<SearchFilterOutput> {
    require_enabled(&app)?;
    state.with(|g| g.parse_search_intent(&query))
}
