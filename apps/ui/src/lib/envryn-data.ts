/**
 * UI-facing types and static configuration.
 *
 * The sample `secrets`, `projects` and `devices` arrays that used to live here
 * are gone: the vault is real now, and shipping mock credentials inside a
 * secrets manager is a way for someone to mistake fake data for their own.
 * Live data comes from `use-vault.ts`, which talks to the Rust core.
 *
 * What remains is genuine configuration -- the set of secret kinds, the fields
 * each kind has, and the category groupings. `typeFields` is hand-maintained
 * for now and should be generated from the Rust `SecretPayload` enum, so that
 * adding a field in Rust cannot be forgotten in the form.
 */

export type SecretType =
  | "API Key"
  | "Token"
  | "Environment"
  | "Database"
  | "SSH"
  | "OAuth"
  | "Webhook"
  | "Note"
  | "Custom";

export type Environment = "Development" | "Staging" | "Production" | "—";

export interface Secret {
  id: string;
  name: string;
  type: SecretType;
  project: string;
  environment: Environment;
  updated: string;
  created: string;
  notes?: string;
  tags?: string[];
  provider?: string;
  /**
   * Empty for records obtained by listing.
   *
   * A list carries no secret material -- that is enforced by the Rust type it
   * comes from. Populated only by an explicit reveal.
   */
  value: string;
  /** Set when a record could not be decrypted, so the row can be flagged rather than silently omitted. */
  damaged?: boolean;
}

export interface Project {
  id: string;
  name: string;
  environments: { name: Environment; count: number }[];
}

export interface Device {
  id: string;
  name: string;
  status: "Trusted" | "Offline" | "Syncing";
  lastSync: string;
  fingerprint: string;
  deviceId: string;
  added: string;
}

export const secretTypes: SecretType[] = [
  "API Key",
  "Environment",
  "Token",
  "Database",
  "SSH",
  "OAuth",
  "Webhook",
  "Note",
  "Custom",
];

export const typeFields: Record<string, string[]> = {
  "API Key": ["Provider"],
  Environment: ["Variable Name"],
  Token: ["Expiration"],
  Database: ["Host", "Port", "Database", "Username"],
  SSH: ["Host", "Username", "Passphrase", "Fingerprint"],
  OAuth: ["Client ID"],
  Webhook: ["Endpoint"],
  Note: [],
  Custom: [],
};

export const categories = {
  "api-tokens": {
    label: "API & tokens",
    description: "Keys, tokens, OAuth credentials, and webhooks used by your apps.",
    types: ["API Key", "Token", "OAuth", "Webhook"] as SecretType[],
  },
  databases: {
    label: "Databases",
    description: "Connection details for databases and hosted data services.",
    types: ["Database"] as SecretType[],
  },
  ssh: {
    label: "SSH",
    description: "Keys and host credentials used to connect to servers.",
    types: ["SSH"] as SecretType[],
  },
  notes: {
    label: "Secure notes",
    description: "Private text such as recovery codes, instructions, or other sensitive notes.",
    types: ["Note"] as SecretType[],
  },
};

export const environmentOrder: Environment[] = ["Development", "Staging", "Production", "—"];
