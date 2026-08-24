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
export const vaultLock = () => call<void>("vault_lock");

export const secretList = () => call<SecretSummary[]>("secret_list");
export const secretSearch = (query: string) => call<SecretSummary[]>("secret_search", { query });
export const secretReveal = (id: string) => call<SecretRecord>("secret_reveal", { id });
export const secretCreate = (input: NewSecret) => call<SecretSummary>("secret_create", { input });
export const secretUpdate = (id: string, update: SecretUpdate) =>
  call<SecretSummary>("secret_update", { id, update });
export const secretDelete = (id: string) => call<void>("secret_delete", { id });
export const secretDuplicates = (id: string) => call<string[]>("secret_duplicates", { id });
