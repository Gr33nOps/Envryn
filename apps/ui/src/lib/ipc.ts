/**
 * The typed IPC contract.
 *
 * These types mirror the Rust types in `crates/envryn-core/src/model.rs`
 * exactly, field name for field name. They are hand-maintained for now; the
 * plan is to generate them with `ts-rs` so there is a single source of truth.
 * Until then, changing a Rust model type means changing this file too.
 *
 * Nothing here performs cryptography or touches storage. The UI sends intents
 * and renders what the Rust core returns.
 */
import { invoke } from "@tauri-apps/api/core";

// --- Rust-mirroring types ---------------------------------------------------

export type SecretKind =
  "ApiKey" | "Token" | "EnvVar" | "Database" | "Ssh" | "OAuth" | "Webhook" | "Note" | "Custom";

export type RustEnvironment = "Development" | "Staging" | "Production" | "Unassigned";

export type SecretPayload =
  | { kind: "ApiKey"; value: string }
  | { kind: "Token"; value: string }
  | { kind: "EnvVar"; key: string; value: string }
  | {
      kind: "Database";
      host: string;
      port: number;
      database: string;
      username: string;
      password: string;
    }
  | {
      kind: "Ssh";
      private_key: string;
      passphrase: string | null;
      host: string | null;
      username: string | null;
    }
  | { kind: "OAuth"; client_id: string; client_secret: string }
  | { kind: "Webhook"; endpoint: string; secret: string }
  | { kind: "Note"; body: string }
  | { kind: "Custom"; fields: { label: string; value: string }[] };

/**
 * What listing returns.
 *
 * Has no field capable of holding secret material -- that is deliberate and
 * mirrors the Rust type. Obtaining a value requires `secretReveal`.
 */
export interface SecretSummary {
  id: string;
  name: string;
  kind: SecretKind;
  project: string;
  environment: RustEnvironment;
  provider: string | null;
  tags: string[];
  has_notes: boolean;
  created_ms: number;
  updated_ms: number;
  rotated_ms: number | null;
}

/** A full record, secret material included. Only ever from `secretReveal`. */
export interface SecretRecord extends Omit<SecretSummary, "has_notes" | "kind"> {
  payload: SecretPayload;
  notes: string | null;
}

export interface NewSecret {
  name: string;
  project: string;
  environment: RustEnvironment;
  payload: SecretPayload;
  notes?: string | null;
  tags?: string[];
  provider?: string | null;
}

export interface SecretUpdate {
  name?: string;
  project?: string;
  environment?: RustEnvironment;
  payload?: SecretPayload;
  notes?: string | null;
  tags?: string[];
  provider?: string | null;
  mark_rotated?: boolean;
}

export interface VaultStatus {
  exists: boolean;
  unlocked: boolean;
  platform_protection_available: boolean;
  platform_protection_enabled: boolean;
}

export interface AppSettings {
  auto_lock_minutes: number;
  clipboard_clear_seconds: number;
  ai_enabled: boolean;
}

export interface RestoreSummary {
  restored: number;
}

export interface TrustedDevice {
  device_id: string;
  fingerprint_hex: string;
  name: string;
  paired_ms: number;
  last_sync_ms: number | null;
}

export interface OwnIdentity {
  device_id: string;
  fingerprint_display: string;
  fingerprint_hex: string;
}

export interface DiscoveredPeer {
  device_id: string;
  fingerprint_hex: string;
  addresses: string[];
  port: number;
}

export interface SyncSummary {
  records_applied: number;
  /** Genuine concurrent edits detected this sync (INV-109) -- the losing
   * side of each was preserved, not discarded; see conflictListAll. */
  conflicts: number;
}

/** The decrypted losing side of a genuine concurrent edit (INV-109). */
export interface ConflictSummary {
  conflict_id: string;
  record: SecretRecord;
}

export interface PairingHostStarted {
  address: string;
  port: number;
  code: string | null;
  device_id: string;
  fingerprint_display: string;
}

export interface PairingSasReady {
  sas: string;
  peer_device_id: string;
  peer_fingerprint_display: string;
}

export interface PairingFailed {
  message: string;
}

export interface PairingComplete {
  peer_device_id: string;
}

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
  } catch (raw) {
    if (isIpcErrorShape(raw)) throw new IpcError(raw.code, raw.message);
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

export interface AiStatus {
  enabled_in_settings: boolean;
  model_downloaded: boolean;
  model_name: string;
  engine_running: boolean;
}

export interface ClassificationOutput {
  kind: SecretKind;
  provider: string | null;
  confidence: number;
}

export interface NameSuggestionOutput {
  name: string;
}

export interface EnvNameEntry {
  name: string;
  kind: SecretKind;
}

export interface EnvNameClassificationOutput {
  names: EnvNameEntry[];
}

export interface ExtractedField {
  label: string;
  value: string;
}

export interface ExtractedFieldsOutput {
  fields: ExtractedField[];
}

export interface SearchFilterOutput {
  project: string | null;
  environment: RustEnvironment | null;
  kind: SecretKind | null;
  tags: string[];
  text: string | null;
}

// Not AI -- plain known-prefix/shape matching that works with no model
// installed. See src-tauri/src/ai.rs's module doc for why this one command
// is not gated by `ai_enabled`.
export interface DeterministicMatch {
  kind: SecretKind;
  provider: string | null;
}
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
