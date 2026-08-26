/**
 * The typed IPC contract.
 *
 * The data types are generated directly from Rust (`packages/contract`,
 * `ts-rs`) -- see that package's own doc comment for how to regenerate them.
 * Only the command wrappers below, `IpcError`, and `isTauri` are
 * hand-maintained here; they are UI-side glue over the wire, not the wire
 * shape itself, so there is nothing for ts-rs to generate for them.
 *
 * Nothing here performs cryptography or touches storage. The UI sends intents
 * and renders what the Rust core returns.
 */
import { invoke } from "@tauri-apps/api/core";
import type {
  AiStatus,
  AppSettings,
  ClassificationOutput,
  ConflictSummary,
  DeterministicMatch,
  DiscoveredPeer,
  EnvNameClassificationOutput,
  ExtractedFieldsOutput,
  NameSuggestionOutput,
  NewSecret,
  OwnIdentity,
  PairingComplete,
  PairingFailed,
  PairingHostStarted,
  PairingSasReady,
  RestoreSummary,
  SearchFilterOutput,
  SecretRecord,
  SecretSummary,
  SecretUpdate,
  SyncSummary,
  TrustedDevice,
  VaultStatus,
} from "@envryn/contract";

export type {
  AiStatus,
  AppSettings,
  ClassificationOutput,
  ConflictSummary,
  DeterministicMatch,
  DiscoveredPeer,
  EnvNameClassificationOutput,
  EnvNameEntry,
  ExtractedField,
  ExtractedFieldsOutput,
  NameSuggestionOutput,
  NewSecret,
  OwnIdentity,
  PairingComplete,
  PairingFailed,
  PairingHostStarted,
  PairingSasReady,
  RestoreSummary,
  RustEnvironment,
  SearchFilterOutput,
  SecretKind,
  SecretPayload,
  SecretRecord,
  SecretSummary,
  SecretUpdate,
  SyncSummary,
  TrustedDevice,
  VaultStatus,
} from "@envryn/contract";

// --- Errors -----------------------------------------------------------------

export type IpcErrorCode =
  | "auth_failed"
  | "locked"
  | "vault_exists"
  | "vault_not_found"
  | "not_found"
  | "invalid_input"
  | "unsupported_version"
  | "decryption_failed"
  | "platform_unavailable"
  | "ai_unavailable"
  | "internal";

export class IpcError extends Error {
  constructor(
    public readonly code: IpcErrorCode,
    message: string,
  ) {
    super(message);
    this.name = "IpcError";
  }
}

function isIpcErrorShape(value: unknown): value is { code: IpcErrorCode; message: string } {
  return (
    typeof value === "object" &&
    value !== null &&
    "code" in value &&
    typeof (value as { code: unknown }).code === "string"
  );
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error_) {
    if (isIpcErrorShape(error_)) throw new IpcError(error_.code, error_.message);
    // An unrecognised rejection means something went wrong below our contract.
    // Report it generically rather than surfacing an unknown payload, which
    // could carry internal detail.
    throw new IpcError("internal", "Something went wrong. Your vault is unaffected.");
  }
}

/**
 * Whether we are running inside Tauri.
 *
 * Used to fail loudly in a browser rather than silently falling back to mock
 * data. A vault UI that appears to work without a vault behind it is a way to
 * lose real secrets.
 */
export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

// --- Commands ---------------------------------------------------------------

export const vaultStatus = () => call<VaultStatus>("vault_status");
export const vaultCreate = (password: string) => call<void>("vault_create", { password });
export const vaultUnlock = (password: string) => call<void>("vault_unlock", { password });
export const vaultUnlockWithPlatform = () => call<void>("vault_unlock_with_platform");
export const vaultLock = () => call<void>("vault_lock");
export const vaultChangePassword = (currentPassword: string, newPassword: string) =>
  call<void>("vault_change_password", { currentPassword, newPassword });
export const vaultEnablePlatformProtection = (password: string) =>
  call<void>("vault_enable_platform_protection", { password });
export const vaultDisablePlatformProtection = () => call<void>("vault_disable_platform_protection");

export const secretList = () => call<SecretSummary[]>("secret_list");
export const secretSearch = (query: string) => call<SecretSummary[]>("secret_search", { query });
export const secretReveal = (id: string) => call<SecretRecord>("secret_reveal", { id });
export const secretCreate = (input: NewSecret) => call<SecretSummary>("secret_create", { input });
export const secretUpdate = (id: string, update: SecretUpdate) =>
  call<SecretSummary>("secret_update", { id, update });
export const secretDelete = (id: string) => call<void>("secret_delete", { id });
export const secretDuplicates = (id: string) => call<string[]>("secret_duplicates", { id });

/**
 * Copy a value to the OS clipboard.
 *
 * Routed through Rust rather than `navigator.clipboard`: only the native call
 * can tag the write so Windows clipboard history and cloud clipboard sync
 * skip it, and only a timer in the Rust process keeps clearing it on schedule
 * regardless of what happens to this webview's own JS event loop.
 */
export const clipboardCopy = (value: string) => call<void>("clipboard_copy", { value });

export const settingsGet = () => call<AppSettings>("settings_get");
export const settingsSet = (settings: AppSettings) =>
  call<AppSettings>("settings_set", { settings });

export const backupCreate = (path: string, password: string) =>
  call<void>("backup_create", { path, password });
export const backupRestore = (path: string, backupPassword: string, newMasterPassword: string) =>
  call<RestoreSummary>("backup_restore", {
    path,
    backupPassword,
    newMasterPassword,
  });

// --- Sync: identity, trusted devices, discovery, manual sync ----------------

export const deviceIdentity = () => call<OwnIdentity>("device_identity");

export const trustedDeviceList = () => call<TrustedDevice[]>("trusted_device_list");
export const trustedDeviceRename = (deviceId: string, name: string) =>
  call<TrustedDevice>("trusted_device_rename", { deviceId, name });
export const trustedDeviceRevoke = (deviceId: string) =>
  call<void>("trusted_device_revoke", { deviceId });

export const discoveryBrowse = () => call<DiscoveredPeer[]>("discovery_browse");

export const syncNow = (address: string, port: number) =>
  call<SyncSummary>("sync_now", { address, port });
export const syncListenStart = () => call<number>("sync_listen_start");
export const syncListenStop = () => call<void>("sync_listen_stop");

// --- Sync conflicts (INV-109) ------------------------------------------------
//
// A genuine concurrent edit is never silently discarded: the Hlc-newer side
// wins and becomes the live value (what secretList/secretReveal show), but
// the losing side is preserved here until the user reviews it.

export const conflictCount = () => call<number>("conflict_count");
export const conflictListAll = () => call<ConflictSummary[]>("conflict_list_all");
export const secretConflicts = (id: string) => call<ConflictSummary[]>("secret_conflicts", { id });
export const conflictRecover = (conflictId: string) =>
  call<SecretSummary>("conflict_recover", { conflictId });
export const conflictDiscard = (conflictId: string) =>
  call<void>("conflict_discard", { conflictId });

// --- AI -----------------------------------------------------------------------
//
// Off by default (`AppSettings.ai_enabled`). Every command below fails with
// `ai_unavailable` if the setting is off or the local worker isn't running --
// see src-tauri/src/ai.rs. Nothing here is on the path of any vault
// operation: unlock, create, edit, sync, and backup all work with AI
// disabled or never started.

// Not AI -- plain known-prefix/shape matching that works with no model
// installed. See src-tauri/src/ai.rs's module doc for why this one command
// is not gated by `ai_enabled`.
export const classifyDeterministic = (value: string) =>
  call<DeterministicMatch | null>("classify_deterministic", { value });

export const aiStatus = () => call<AiStatus>("ai_status");
export const aiDownloadModel = () => call<void>("ai_download_model");
export const aiStart = () => call<void>("ai_start");
export const aiStop = () => call<void>("ai_stop");

export const aiClassifyPastedValue = (value: string) =>
  call<ClassificationOutput>("ai_classify_pasted_value", { value });
export const aiSuggestName = (value: string, provider: string | null) =>
  call<NameSuggestionOutput>("ai_suggest_name", { value, provider });
export const aiClassifyEnvNames = (names: string[]) =>
  call<EnvNameClassificationOutput>("ai_classify_env_names", { names });
export const aiExtractStructuredFields = (block: string) =>
  call<ExtractedFieldsOutput>("ai_extract_structured_fields", { block });
export const aiParseSearchIntent = (query: string) =>
  call<SearchFilterOutput>("ai_parse_search_intent", { query });

// --- Pairing ------------------------------------------------------------------
//
// `pairing_host_start`/`pairing_join_start` return once a listener/connection
// exists; the actual outcome (a verification code to compare, success, or
// failure) arrives later as a `pairing://*` event -- see `listenPairingEvents`
// below. This mirrors the Rust side: the human confirming the code is what
// authorises the vault key transfer, not the command call itself.

export const pairingHostStart = (manual: boolean) =>
  call<PairingHostStarted>("pairing_host_start", { manual });
export const pairingJoinStart = (address: string, port: number, code: string | null) =>
  call<void>("pairing_join_start", { address, port, code });
export const pairingConfirm = (secret: string) => call<void>("pairing_confirm", { secret });
export const pairingCancel = () => call<void>("pairing_cancel");

/**
 * Subscribe to the three pairing lifecycle events emitted from the Rust
 * background thread driving an in-progress pairing session. Returns an
 * unsubscribe function; callers should invoke it on unmount so a closed
 * pairing modal does not keep a stale listener alive.
 */
export async function listenPairingEvents(handlers: {
  onSasReady: (event: PairingSasReady) => void;
  onFailed: (event: PairingFailed) => void;
  onComplete: (event: PairingComplete) => void;
}): Promise<() => void> {
  const { listen } = await import("@tauri-apps/api/event");
  const unlisten = await Promise.all([
    listen<PairingSasReady>("pairing://sas-ready", (e) => handlers.onSasReady(e.payload)),
    listen<PairingFailed>("pairing://failed", (e) => handlers.onFailed(e.payload)),
    listen<PairingComplete>("pairing://complete", (e) => handlers.onComplete(e.payload)),
  ]);
  return () => unlisten.forEach((fn) => fn());
}
