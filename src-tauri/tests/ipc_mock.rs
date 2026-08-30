//! Real IPC dispatch tests, using Tauri's own `MockRuntime` (`tauri::test`,
//! gated behind the `test` feature -- see `Cargo.toml`'s `[dev-dependencies]`)
//! rather than calling the command functions as plain Rust. `get_ipc_response`
//! drives a command by its real string name through the exact dispatch path
//! `generate_handler!` wires up for the live app, so these prove the
//! `#[tauri::command]` boundary itself -- argument deserialization, state
//! injection, error mapping to `IpcError` -- not just the logic behind it
//! (which `envryn-core` already covers on its own, with no Tauri involved).
//!
//! This lives as an external integration test target rather than an internal
//! `#[cfg(test)] mod tests` inside `lib.rs`, because it must, since
//! `build.rs`'s Common-Controls-v6 manifest fix for `cargo test` binaries
//! (`embed_resource::compile_for_tests`, see `build.rs`'s comment) only
//! applies to Cargo's `test`-kind targets -- a `#[cfg(test)] mod` inside the
//! `lib` target does not qualify, and without that manifest the process fails
//! to even start (`STATUS_ENTRYPOINT_NOT_FOUND`, resolving `comctl32.dll`'s
//! v6-only `SetWindowSubclass`/etc. against the legacy v5 side-by-side
//! assembly). Being external is also why `lib.rs` declares `ai`/`ipc`/
//! `settings`/`sync` as `pub mod` rather than the private `mod` an
//! application-only crate would otherwise use.
//!
//! Deliberately excluded: every command that takes a bare `tauri::AppHandle`
//! (`vault_status`, `vault_create`, `vault_unlock`, `settings_get`,
//! `settings_set`, `sync::device_identity`, ...). `AppHandle` carries the
//! `#[default_runtime(crate::Wry, wry)]` attribute, so with this crate's
//! default `wry` feature on, a bare `AppHandle` in a command signature always
//! means `AppHandle<Wry>` -- never generic over `R`. `generate_handler!`
//! against a `MockRuntime`-based builder then can't bind it (`AppHandle<Wry>`
//! is not `AppHandle<MockRuntime>`), which is a real, documented Tauri
//! limitation, not a bug in this crate: it fails at compile time with "the
//! trait `Deserialize<'_>` is not implemented for `AppHandle`" if attempted.
//! Making those commands generic over `R: tauri::Runtime` purely to satisfy a
//! test harness would touch every trust-boundary command in `ipc.rs`,
//! `settings.rs`, and `sync.rs` -- a real production change to the audited
//! IPC surface, not additive test scaffolding, so it is left alone. The tests
//! below cover every command whose only injected argument is `State`, which
//! is the majority of the surface and the part that actually touches
//! `VaultState`; the vault itself is created directly via
//! `envryn_core::vault::Vault` at a throwaway temp path (bypassing the
//! `AppHandle`-dependent `vault_create` command, not the crypto it wraps) and
//! installed into `VaultState`'s public `Mutex<Option<Vault>>` field the same
//! way `vault_create`/`vault_unlock` populate it.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::sync::atomic::{AtomicU64, Ordering};

use envryn_core::crypto::kdf::KdfParams;
use envryn_core::vault::Vault;
use envryn_lib::ai::{self, AiState};
use envryn_lib::ipc::{self, VaultState};
use envryn_lib::sync::{PairingState, SyncListenState};
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{get_ipc_response, mock_builder, mock_context, noop_assets, MockRuntime};
use tauri::webview::InvokeRequest;
use tauri::{App, Manager, WebviewWindow, WebviewWindowBuilder};
use zeroize::Zeroizing;

/// Builds a mock app with every command these tests dispatch wired up
/// through the real `generate_handler!` path -- state is always managed
/// on the `Builder` before `.build()`, since `App::manage` called after
/// the fact cannot retroactively register a handler with `.invoke_handler`.
fn build_mock_app() -> App<MockRuntime> {
    mock_builder()
        .manage(VaultState::default())
        .manage(PairingState::default())
        .manage(SyncListenState::default())
        .manage(AiState::default())
        .invoke_handler(tauri::generate_handler![
            ai::classify_deterministic,
            ipc::vault_lock,
            ipc::secret_list,
            ipc::secret_search,
            ipc::project_list,
            ipc::project_create,
            ipc::project_rename,
            ipc::secret_reveal,
            ipc::secret_create,
            ipc::secret_update,
            ipc::secret_delete,
            ipc::secret_duplicates,
            ipc::secret_conflicts,
            ipc::conflict_count,
            ipc::conflict_list_all,
            ipc::conflict_recover,
            ipc::conflict_discard,
        ])
        .build(mock_context(noop_assets()))
        .expect("mock app builds")
}

/// Deletes its directory on drop (including on test panic/unwind), so a
/// failing assertion doesn't leave a stray temp vault file behind.
struct CleanupDir(std::path::PathBuf);

impl Drop for CleanupDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn unique_temp_dir(label: &str) -> std::path::PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock is after the unix epoch")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "envryn-ipc-test-{label}-{}-{n}-{nanos}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("temp test dir creates");
    dir
}

/// Builds a mock app with every `VaultState`-only command wired, plus a
/// real, already-unlocked `Vault` installed into that state -- created at
/// `KdfParams::MINIMUM` (still a real Argon2id derivation, just not the
/// ~700ms production calibration) since these tests exercise IPC
/// dispatch, not KDF strength, which `envryn-core`'s own crypto tests
/// already cover.
fn test_app_with_unlocked_vault() -> (App<MockRuntime>, CleanupDir) {
    let dir = unique_temp_dir("vault");
    let db_path = dir.join("envryn.db");

    let password = Zeroizing::new("correct horse battery staple".to_string());
    let mut vault = Vault::create(&db_path, &password, KdfParams::MINIMUM).expect("vault creates");
    vault
        .set_local_device_id(1)
        .expect("device id sets on an unlocked vault");

    let app = build_mock_app();
    *app.state::<VaultState>()
        .0
        .lock()
        .expect("vault state mutex is not poisoned") = Some(vault);

    (app, CleanupDir(dir))
}

fn call(
    webview: &WebviewWindow<MockRuntime>,
    cmd: &str,
    args: serde_json::Value,
) -> Result<serde_json::Value, serde_json::Value> {
    get_ipc_response(
        webview,
        InvokeRequest {
            cmd: cmd.into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: "http://tauri.localhost".parse().expect("static url parses"),
            body: InvokeBody::Json(args),
            headers: Default::default(),
            invoke_key: tauri::test::INVOKE_KEY.to_string(),
        },
    )
    .map(|b| {
        b.deserialize::<serde_json::Value>()
            .expect("ipc response is valid json")
    })
}

#[test]
fn classify_deterministic_dispatches_with_no_state_at_all() {
    let app = build_mock_app();
    let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("mock webview builds");

    // A command with no `State`/`AppHandle` argument at all -- the
    // simplest possible proof that `generate_handler!`'s dispatch path
    // (argument deserialization, response serialization) works in this
    // crate before layering state injection on top of it.
    let hit = call(
        &webview,
        "classify_deterministic",
        serde_json::json!({ "value": "ghp_1234567890abcdef1234567890abcdef1234" }),
    )
    .expect("classify_deterministic succeeds");
    assert_eq!(hit["kind"], "Token");

    let miss = call(
        &webview,
        "classify_deterministic",
        serde_json::json!({ "value": "just some plain text" }),
    )
    .expect("classify_deterministic succeeds");
    assert!(miss.is_null(), "an unrecognized value must match nothing");
}

#[test]
fn project_create_then_list_round_trips_without_a_placeholder_secret() {
    let (app, _cleanup) = test_app_with_unlocked_vault();
    let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("mock webview builds");

    let created = call(
        &webview,
        "project_create",
        serde_json::json!({ "name": "Mobile API" }),
    )
    .expect("project_create succeeds");
    assert_eq!(created["name"], "Mobile API");
    assert!(created["id"].as_str().is_some_and(|id| !id.is_empty()));

    let projects =
        call(&webview, "project_list", serde_json::json!({})).expect("project_list succeeds");
    assert_eq!(projects.as_array().map(Vec::len), Some(1));
    assert_eq!(projects[0]["name"], "Mobile API");

    let secrets =
        call(&webview, "secret_list", serde_json::json!({})).expect("secret_list succeeds");
    assert_eq!(secrets.as_array().map(Vec::len), Some(0));
}

#[test]
fn secret_create_then_list_then_reveal_round_trips_through_real_ipc_dispatch() {
    let (app, _cleanup) = test_app_with_unlocked_vault();
    let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("mock webview builds");

    let created = call(
        &webview,
        "secret_create",
        serde_json::json!({
            "input": {
                "name": "OPENAI_API_KEY",
                "project": "Test Project",
                "environment": "Development",
                "payload": { "kind": "ApiKey", "value": "sk-test-not-a-real-key" },
                "tags": []
            }
        }),
    )
    .expect("secret_create succeeds");
    assert_eq!(created["name"], "OPENAI_API_KEY");
    let id = created["id"]
        .as_str()
        .expect("summary carries an id")
        .to_string();

    let listed =
        call(&webview, "secret_list", serde_json::json!({})).expect("secret_list succeeds");
    let secrets = listed.as_array().expect("secret_list returns a JSON array");
    assert_eq!(secrets.len(), 1);
    assert_eq!(secrets[0]["name"], "OPENAI_API_KEY");
    assert!(
        secrets[0].get("value").is_none(),
        "a summary must carry no field capable of holding the secret value"
    );

    let revealed = call(&webview, "secret_reveal", serde_json::json!({ "id": id }))
        .expect("secret_reveal succeeds");
    assert_eq!(revealed["payload"]["kind"], "ApiKey");
    assert_eq!(revealed["payload"]["value"], "sk-test-not-a-real-key");
}

#[test]
fn secret_delete_then_list_shows_nothing_and_reveal_fails() {
    let (app, _cleanup) = test_app_with_unlocked_vault();
    let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("mock webview builds");

    let created = call(
        &webview,
        "secret_create",
        serde_json::json!({
            "input": {
                "name": "TEMP_TOKEN",
                "project": "Test Project",
                "environment": "Development",
                "payload": { "kind": "Token", "value": "throwaway" },
                "tags": []
            }
        }),
    )
    .expect("secret_create succeeds");
    let id = created["id"]
        .as_str()
        .expect("summary carries an id")
        .to_string();

    call(
        &webview,
        "secret_delete",
        serde_json::json!({ "id": id.clone() }),
    )
    .expect("secret_delete succeeds");

    let listed =
        call(&webview, "secret_list", serde_json::json!({})).expect("secret_list succeeds");
    assert_eq!(
        listed.as_array().expect("array").len(),
        0,
        "a deleted secret must not appear in the list"
    );

    let err = call(&webview, "secret_reveal", serde_json::json!({ "id": id }))
        .expect_err("revealing a deleted secret must fail");
    assert_eq!(err["code"], "not_found");
}

#[test]
fn vault_lock_clears_state_so_every_secret_command_reports_locked() {
    let (app, _cleanup) = test_app_with_unlocked_vault();
    let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("mock webview builds");

    call(&webview, "vault_lock", serde_json::json!({})).expect("vault_lock succeeds");

    let err = call(&webview, "secret_list", serde_json::json!({}))
        .expect_err("secret_list must fail once the vault is locked");
    assert_eq!(err["code"], "locked");
}

#[test]
fn secret_conflicts_and_conflict_count_start_empty_on_a_fresh_vault() {
    let (app, _cleanup) = test_app_with_unlocked_vault();
    let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("mock webview builds");

    let count =
        call(&webview, "conflict_count", serde_json::json!({})).expect("conflict_count succeeds");
    assert_eq!(count, 0);

    let all = call(&webview, "conflict_list_all", serde_json::json!({}))
        .expect("conflict_list_all succeeds");
    assert_eq!(all.as_array().expect("array").len(), 0);
}
