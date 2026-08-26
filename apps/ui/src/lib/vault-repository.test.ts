import { describe, expect, it, vi, beforeEach } from "vitest";
import {
  payloadToDisplay,
  tauriVaultRepository,
  toUiEnvironment,
  KIND_TO_TYPE,
} from "./vault-repository";
import type { SecretPayload, SecretSummary, TrustedDevice } from "@envryn/contract";

const isTauri = vi.fn(() => true);
const secretCreate = vi.fn();
const secretUpdate = vi.fn();
const secretList = vi.fn();
const secretReveal = vi.fn();
const trustedDeviceList = vi.fn();

vi.mock("./ipc", async () => {
  const actual = await vi.importActual<typeof import("./ipc")>("./ipc");
  return {
    ...actual,
    isTauri: () => isTauri(),
    secretCreate: (...args: unknown[]) => secretCreate(...args),
    secretUpdate: (...args: unknown[]) => secretUpdate(...args),
    secretList: (...args: unknown[]) => secretList(...args),
    secretReveal: (...args: unknown[]) => secretReveal(...args),
    trustedDeviceList: (...args: unknown[]) => trustedDeviceList(...args),
  };
});

beforeEach(() => {
  isTauri.mockReturnValue(true);
  secretCreate.mockReset();
  secretUpdate.mockReset();
  secretList.mockReset();
  secretReveal.mockReset();
  trustedDeviceList.mockReset();
});

// --- payloadToDisplay: pure, exported, one real credential shape at a time --

describe("payloadToDisplay", () => {
  it("shows the raw value for ApiKey, Token, and EnvVar", () => {
    expect(payloadToDisplay({ kind: "ApiKey", value: "sk-live-abc" })).toBe("sk-live-abc");
    expect(payloadToDisplay({ kind: "Token", value: "ghp_abc" })).toBe("ghp_abc");
    expect(payloadToDisplay({ kind: "EnvVar", key: "DATABASE_URL", value: "abc" })).toBe("abc");
  });

  it("shows the note body for Note", () => {
    expect(payloadToDisplay({ kind: "Note", body: "call mom" })).toBe("call mom");
  });

  it("composes a connection string for Database", () => {
    const payload: SecretPayload = {
      kind: "Database",
      host: "db.example.com",
      port: 5432,
      database: "prod",
      username: "admin",
      password: "hunter2",
    };
    expect(payloadToDisplay(payload)).toBe("admin@db.example.com:5432/prod");
  });

  it("shows the private key for Ssh", () => {
    const payload: SecretPayload = {
      kind: "Ssh",
      private_key: "-----BEGIN KEY-----",
      passphrase: null,
      host: null,
      username: null,
    };
    expect(payloadToDisplay(payload)).toBe("-----BEGIN KEY-----");
  });

  it("shows the client secret for OAuth", () => {
    expect(payloadToDisplay({ kind: "OAuth", client_id: "id", client_secret: "shh" })).toBe("shh");
  });

  it("shows the signing secret for Webhook", () => {
    expect(payloadToDisplay({ kind: "Webhook", endpoint: "https://x", secret: "whsec_abc" })).toBe(
      "whsec_abc",
    );
  });

  it("joins every field as label: value for Custom", () => {
    const payload: SecretPayload = {
      kind: "Custom",
      fields: [
        { label: "Host", value: "1.2.3.4" },
        { label: "Port", value: "22" },
      ],
    };
    expect(payloadToDisplay(payload)).toBe("Host: 1.2.3.4\nPort: 22");
  });
});

// --- toUiEnvironment: pure, exported ----------------------------------------

describe("toUiEnvironment", () => {
  it("maps Unassigned to the display dash", () => {
    expect(toUiEnvironment("Unassigned")).toBe("—");
  });

  it("passes real environments through unchanged", () => {
    expect(toUiEnvironment("Development")).toBe("Development");
    expect(toUiEnvironment("Staging")).toBe("Staging");
    expect(toUiEnvironment("Production")).toBe("Production");
  });
});

describe("KIND_TO_TYPE", () => {
  it("covers every SecretKind with a real display label", () => {
    expect(Object.keys(KIND_TO_TYPE).sort()).toEqual(
      ["ApiKey", "Custom", "Database", "EnvVar", "Note", "OAuth", "Ssh", "Token", "Webhook"].sort(),
    );
  });
});

// --- createSecret: exercises toPayload's real branching, including the
// Custom-kind fallback and the multi-field-kind-becomes-Note behaviour both
// documented in the source. ---------------------------------------------------

function fakeSummary(overrides: Partial<SecretSummary> = {}): SecretSummary {
  return {
    id: "id-1",
    name: "OPENAI_API_KEY",
    kind: "ApiKey",
    project: "Rescripto",
    environment: "Development",
    provider: null,
    tags: [],
    has_notes: false,
    created_ms: Date.now(),
    updated_ms: Date.now(),
    rotated_ms: null,
    ...overrides,
  };
}

describe("tauriVaultRepository.createSecret", () => {
  it("sends an ApiKey payload for an API Key type", async () => {
    secretCreate.mockResolvedValue(fakeSummary());
    await tauriVaultRepository.createSecret({
      name: "OPENAI_API_KEY",
      type: "API Key",
      project: "Rescripto",
      environment: "Development",
      value: "sk-abc",
    } as never);

    expect(secretCreate).toHaveBeenCalledWith(
      expect.objectContaining({ payload: { kind: "ApiKey", value: "sk-abc" } }),
    );
  });

  it("sends a Custom payload with the given fields when the type is Custom and fields are supplied", async () => {
    secretCreate.mockResolvedValue(fakeSummary({ kind: "Custom" }));
    await tauriVaultRepository.createSecret({
      name: "Server config",
      type: "Custom",
      project: "",
      environment: "—",
      value: "unused",
      customFields: [{ label: "Host", value: "1.2.3.4" }],
    } as never);

    expect(secretCreate).toHaveBeenCalledWith(
      expect.objectContaining({
        payload: { kind: "Custom", fields: [{ label: "Host", value: "1.2.3.4" }] },
      }),
    );
  });

  it("falls back to a single Value field for Custom with no fields supplied", async () => {
    secretCreate.mockResolvedValue(fakeSummary({ kind: "Custom" }));
    await tauriVaultRepository.createSecret({
      name: "Server config",
      type: "Custom",
      project: "",
      environment: "—",
      value: "just one value",
    } as never);

    expect(secretCreate).toHaveBeenCalledWith(
      expect.objectContaining({
        payload: { kind: "Custom", fields: [{ label: "Value", value: "just one value" }] },
      }),
    );
  });

  it("stores a Database-typed value as a Note, since the form only collects one field", async () => {
    secretCreate.mockResolvedValue(fakeSummary({ kind: "Database" }));
    await tauriVaultRepository.createSecret({
      name: "Prod DB",
      type: "Database",
      project: "",
      environment: "—",
      value: "postgres://user:pass@host/db",
    } as never);

    expect(secretCreate).toHaveBeenCalledWith(
      expect.objectContaining({
        payload: { kind: "Note", body: "postgres://user:pass@host/db" },
      }),
    );
  });

  it("maps the UI dash environment to Unassigned for the Rust side", async () => {
    secretCreate.mockResolvedValue(fakeSummary());
    await tauriVaultRepository.createSecret({
      name: "x",
      type: "API Key",
      project: "",
      environment: "—",
      value: "v",
    } as never);

    expect(secretCreate).toHaveBeenCalledWith(
      expect.objectContaining({ environment: "Unassigned" }),
    );
  });

  it("maps the returned summary back into the UI's Secret shape with an empty value", async () => {
    secretCreate.mockResolvedValue(
      fakeSummary({ name: "OPENAI_API_KEY", provider: "OpenAI", tags: ["prod"] }),
    );
    const secret = await tauriVaultRepository.createSecret({
      name: "OPENAI_API_KEY",
      type: "API Key",
      project: "Rescripto",
      environment: "Development",
      value: "sk-abc",
    } as never);

    expect(secret.name).toBe("OPENAI_API_KEY");
    expect(secret.provider).toBe("OpenAI");
    expect(secret.tags).toEqual(["prod"]);
    // A summary never carries secret material -- this must always be "".
    expect(secret.value).toBe("");
  });

  it("throws without ever calling the Rust core when Tauri is unavailable", async () => {
    isTauri.mockReturnValue(false);
    await expect(
      tauriVaultRepository.createSecret({
        name: "x",
        type: "API Key",
        project: "",
        environment: "—",
        value: "v",
      } as never),
    ).rejects.toThrow();
    expect(secretCreate).not.toHaveBeenCalled();
  });
});

describe("tauriVaultRepository.listSecrets", () => {
  it("maps every summary in the list through the same Secret shape", async () => {
    secretList.mockResolvedValue([
      fakeSummary({ id: "a" }),
      fakeSummary({ id: "b", kind: "Database" }),
    ]);
    const secrets = await tauriVaultRepository.listSecrets();
    expect(secrets.map((s) => s.id)).toEqual(["a", "b"]);
    expect(secrets[1]?.type).toBe("Database");
  });
});

describe("tauriVaultRepository.revealSecret", () => {
  it("returns the displayable text for the revealed payload", async () => {
    secretReveal.mockResolvedValue({
      payload: { kind: "ApiKey", value: "sk-live-real" },
    });
    const value = await tauriVaultRepository.revealSecret("id-1");
    expect(value).toBe("sk-live-real");
  });
});

describe("tauriVaultRepository.listDevices", () => {
  it("formats the fingerprint as colon-separated uppercase hex", async () => {
    const device: TrustedDevice = {
      device_id: "dev-1",
      fingerprint_hex: "ab12cd34",
      name: "My Laptop",
      paired_ms: Date.now(),
      last_sync_ms: null,
    };
    trustedDeviceList.mockResolvedValue([device]);
    const devices = await tauriVaultRepository.listDevices();
    expect(devices[0]?.fingerprint).toBe("AB:12:CD:34");
    expect(devices[0]?.lastSync).toBe("Never");
    expect(devices[0]?.status).toBe("Trusted");
  });
});
