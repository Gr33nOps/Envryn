import type { Device, Environment, Secret, SecretType } from "./envryn-data";
import * as ipc from "./ipc";

export type CreateSecretInput = Omit<Secret, "id" | "created" | "updated"> & {
  /** Only meaningful when `type` is "Custom" -- see `toPayload`. */
  customFields?: { label: string; value: string }[];
};
export type UpdateSecretInput = Partial<CreateSecretInput>;

/**
 * Persistence contract for the vault UI.
 *
 * Implemented against the Rust core over Tauri IPC. There is deliberately no
 * mock implementation any more: a vault UI that appears to work without a
 * vault behind it is a way for someone to enter real credentials into nothing.
 * Outside Tauri, every call fails loudly.
 */
export interface VaultRepository {
  listSecrets(): Promise<Secret[]>;
  searchSecrets(query: string): Promise<Secret[]>;
  revealSecret(id: string): Promise<string>;
  listDevices(): Promise<Device[]>;
  renameDevice(deviceId: string, name: string): Promise<Device>;
  revokeDevice(deviceId: string): Promise<void>;
  createSecret(input: CreateSecretInput): Promise<Secret>;
  updateSecret(id: string, input: UpdateSecretInput): Promise<Secret>;
  deleteSecret(id: string): Promise<void>;
  duplicatesOf(id: string): Promise<string[]>;
}

// --- Mapping between the Rust model and the UI's display shape --------------
// The UI uses human-readable labels ("API Key"); Rust uses variant names
// ("ApiKey"). Both mappings live here so the boundary is one file.

export const KIND_TO_TYPE: Record<ipc.SecretKind, SecretType> = {
  ApiKey: "API Key",
  Token: "Token",
  EnvVar: "Environment",
  Database: "Database",
  Ssh: "SSH",
  OAuth: "OAuth",
  Webhook: "Webhook",
  Note: "Note",
  Custom: "Custom",
};

const TYPE_TO_KIND: Record<SecretType, ipc.SecretKind> = {
  "API Key": "ApiKey",
  Token: "Token",
  Environment: "EnvVar",
  Database: "Database",
  SSH: "Ssh",
  OAuth: "OAuth",
  Webhook: "Webhook",
  Note: "Note",
  Custom: "Custom",
};

export const toUiEnvironment = (env: ipc.RustEnvironment): Environment =>
  env === "Unassigned" ? "—" : env;

const toRustEnvironment = (env: Environment): ipc.RustEnvironment =>
  env === "—" ? "Unassigned" : env;

/**
 * Build a payload from the UI's single `value` string.
 *
 * Multi-field kinds Database, SSH, and OAuth genuinely need separate inputs,
 * and the form does not collect them yet. Rather than guess at parsing a URL
 * into host/port/user/password -- which would silently mis-file credentials --
 * those kinds are stored as a Note payload until the form is extended, so
 * nothing is lost and nothing is fabricated.
 *
 * `EnvVar`'s `key` (the variable name itself, e.g. `DATABASE_URL`) is distinct
 * from the vault's display `name` in principle, but the form collects only one
 * name field -- so it doubles as both, which is exactly right for a `.env`
 * import (`EnvImportModal.tsx`), where the variable name *is* the only name
 * there is.
 *
 * `Custom` is genuinely multi-field (`SecretPayload`'s `fields: {label,
 * value}[]`) -- `customFields` carries that when the caller already has
 * labeled pairs (`StructuredExtractModal.tsx`). The plain create/edit form
 * only ever collects one value, so without `customFields` a Custom secret
 * becomes one field named "Value" -- a real fix, not a new gap: `TYPE_TO_KIND`
 * already mapped "Custom" to the `Custom` kind, but this function had no
 * `case "Custom"` arm at all, so it silently fell through to `Note` instead.
 */
function toPayload(
  type: SecretType,
  value: string,
  name: string,
  customFields?: { label: string; value: string }[],
): ipc.SecretPayload {
  const kind = TYPE_TO_KIND[type];
  switch (kind) {
    case "ApiKey":
      return { kind: "ApiKey", value };
    case "Token":
      return { kind: "Token", value };
    case "EnvVar":
      return { kind: "EnvVar", key: name, value };
    case "Custom":
      return {
        kind: "Custom",
        fields:
          customFields && customFields.length > 0 ? customFields : [{ label: "Value", value }],
      };
    case "Note":
      return { kind: "Note", body: value };
    default:
      return { kind: "Note", body: value };
  }
}

/**
 * Present a payload as one display string.
 *
 * Used only after an explicit reveal, never in a list.
 */
export function payloadToDisplay(payload: ipc.SecretPayload): string {
  switch (payload.kind) {
    case "ApiKey":
    case "Token":
    case "EnvVar":
      return payload.value;
    case "Note":
      return payload.body;
    case "Database":
      return `${payload.username}@${payload.host}:${payload.port}/${payload.database}`;
    case "Ssh":
      return payload.private_key;
    case "OAuth":
      return payload.client_secret;
    case "Webhook":
      return payload.secret;
    case "Custom":
      return payload.fields.map((f) => `${f.label}: ${f.value}`).join("\n");
  }
}

const RELATIVE = new Intl.RelativeTimeFormat("en", { numeric: "auto" });

function relative(ms: number): string {
  const diff = ms - Date.now();
  const days = Math.round(diff / 86_400_000);
  if (Math.abs(days) >= 1) return RELATIVE.format(days, "day");
  const hours = Math.round(diff / 3_600_000);
  if (Math.abs(hours) >= 1) return RELATIVE.format(hours, "hour");
  const minutes = Math.round(diff / 60_000);
  if (Math.abs(minutes) >= 1) return RELATIVE.format(minutes, "minute");
  return "Just now";
}

const ABSOLUTE = new Intl.DateTimeFormat("en", {
  year: "numeric",
  month: "long",
  day: "numeric",
});

/**
 * Map a summary into the UI's Secret shape.
 *
 * `value` is deliberately the empty string. A summary carries no secret
 * material -- that is the point of the type on the Rust side, and dropping a
 * placeholder in here would quietly undo it. Call `revealSecret` for a value.
 */
function toSecret(summary: ipc.SecretSummary): Secret {
  return {
    id: summary.id,
    name: summary.name,
    type: KIND_TO_TYPE[summary.kind],
    project: summary.project || "Unassigned",
    environment: toUiEnvironment(summary.environment),
    updated: relative(summary.updated_ms),
    created: ABSOLUTE.format(new Date(summary.created_ms)),
    tags: summary.tags,
    ...(summary.provider ? { provider: summary.provider } : {}),
    value: "",
  };
}

/**
 * Colon-separated uppercase hex, matching the format
 * `sync::identity::Fingerprint::to_display_string` produces on the Rust
 * side (and what earlier UI mockups already assumed).
 */
function toDisplayFingerprint(hex: string): string {
  return (hex.match(/.{1,2}/g) ?? [hex]).join(":").toUpperCase();
}

/**
 * Map a trusted-device record into the UI's display shape.
 *
 * `status` is always "Trusted" here -- this call does not check whether the
 * device is currently reachable on the LAN (that needs a several-second mDNS
 * browse, `ipc.discoveryBrowse`, which the devices list should not block on).
 * "Trusted" is still accurate: it is the vault's approval state, independent
 * of whether the device happens to be online right now. `sync.tsx` layers
 * live reachability on top via a separate browse when the user asks to sync.
 */
function toDevice(device: ipc.TrustedDevice): Device {
  return {
    id: device.device_id,
    name: device.name,
    status: "Trusted",
    lastSync: device.last_sync_ms ? relative(device.last_sync_ms) : "Never",
    fingerprint: toDisplayFingerprint(device.fingerprint_hex),
    deviceId: device.device_id,
    added: ABSOLUTE.format(new Date(device.paired_ms)),
  };
}

function requireTauri(): void {
  if (!ipc.isTauri()) {
    throw new ipc.IpcError(
      "internal",
      "Envryn's vault is unavailable. Run the desktop application rather than a browser.",
    );
  }
}

export const tauriVaultRepository: VaultRepository = {
  async listSecrets() {
    requireTauri();
    return (await ipc.secretList()).map(toSecret);
  },

  async searchSecrets(query) {
    requireTauri();
    return (await ipc.secretSearch(query)).map(toSecret);
  },

  async revealSecret(id) {
    requireTauri();
    const record = await ipc.secretReveal(id);
    return payloadToDisplay(record.payload);
  },

  async listDevices() {
    requireTauri();
    return (await ipc.trustedDeviceList()).map(toDevice);
  },

  async renameDevice(deviceId, name) {
    requireTauri();
    return toDevice(await ipc.trustedDeviceRename(deviceId, name));
  },

  async revokeDevice(deviceId) {
    requireTauri();
    await ipc.trustedDeviceRevoke(deviceId);
  },

  async createSecret(input) {
    requireTauri();
    const summary = await ipc.secretCreate({
      name: input.name,
      project: input.project,
      environment: toRustEnvironment(input.environment),
      payload: toPayload(input.type, input.value, input.name, input.customFields),
      notes: input.notes ?? null,
      tags: input.tags ?? [],
      provider: input.provider ?? null,
    });
    return toSecret(summary);
  },

  async updateSecret(id, input) {
    requireTauri();
    const update: ipc.SecretUpdate = {};
    if (input.name !== undefined) update.name = input.name;
    if (input.project !== undefined) update.project = input.project;
    if (input.environment !== undefined) {
      update.environment = toRustEnvironment(input.environment);
    }
    if (input.value !== undefined && input.type !== undefined) {
      update.payload = toPayload(input.type, input.value, input.name ?? "", input.customFields);
    }
    if (input.notes !== undefined) update.notes = input.notes;
    if (input.tags !== undefined) update.tags = input.tags;
    if (input.provider !== undefined) update.provider = input.provider;

    return toSecret(await ipc.secretUpdate(id, update));
  },

  async deleteSecret(id) {
    requireTauri();
    await ipc.secretDelete(id);
  },

  async duplicatesOf(id) {
    requireTauri();
    return ipc.secretDuplicates(id);
  },
};

export const vaultRepository = tauriVaultRepository;
