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
  value: string;
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

export const secrets: Secret[] = [
  {
    id: "s1",
    name: "GROQ_API_KEY",
    type: "API Key",
    project: "Rescripto",
    environment: "Development",
    updated: "2 days ago",
    created: "August 20, 2026",
    notes: "Groq development key",
    provider: "Groq",
    tags: ["ai", "llm"],
    value: "gsk_9dK2mQ4vTz81LpXw0aBn7Rc5",
  },
  {
    id: "s2",
    name: "SUPABASE_SERVICE_ROLE_KEY",
    type: "API Key",
    project: "Rescripto",
    environment: "Production",
    updated: "5 days ago",
    created: "July 2, 2026",
    provider: "Supabase",
    value: "sbp_f81c0a4e77d9b2c3e5a1f6d4",
  },
  {
    id: "s3",
    name: "SUPABASE_URL",
    type: "Environment",
    project: "Rescripto",
    environment: "Development",
    updated: "5 days ago",
    created: "July 2, 2026",
    value: "https://kxqzvbn.supabase.co",
  },
  {
    id: "s4",
    name: "DATABASE_URL",
    type: "Database",
    project: "NameVetta",
    environment: "Production",
    updated: "Yesterday",
    created: "June 14, 2026",
    notes: "Primary Postgres cluster",
    value: "postgres://app:9fTz@db.namevetta.io:5432/main",
  },
  {
    id: "s5",
    name: "GitHub Personal Token",
    type: "Token",
    project: "Personal",
    environment: "—",
    updated: "1 week ago",
    created: "May 3, 2026",
    provider: "GitHub",
    value: "ghp_A1b2C3d4E5f6G7h8I9j0",
  },
  {
    id: "s6",
    name: "VPS Production",
    type: "SSH",
    project: "Infrastructure",
    environment: "Production",
    updated: "3 weeks ago",
    created: "March 11, 2026",
    value: "-----BEGIN OPENSSH PRIVATE KEY-----",
  },
  {
    id: "s7",
    name: "JWT_SECRET",
    type: "Custom",
    project: "Rescripto",
    environment: "Development",
    updated: "3 weeks ago",
    created: "March 30, 2026",
    value: "b81f0e2c7a5d43918cf6",
  },
  {
    id: "s8",
    name: "STRIPE_WEBHOOK_SECRET",
    type: "Webhook",
    project: "NameVetta",
    environment: "Production",
    updated: "4 days ago",
    created: "April 18, 2026",
    provider: "Stripe",
    value: "whsec_2Kd91jaLm4Xp0Qv",
  },
  {
    id: "s9",
    name: "Google OAuth Client",
    type: "OAuth",
    project: "MyGameList",
    environment: "Production",
    updated: "6 days ago",
    created: "April 2, 2026",
    provider: "Google",
    value: "GOCSPX-4kd82ndkQ01mfPz",
  },
  {
    id: "s10",
    name: "OPENAI_API_KEY",
    type: "API Key",
    project: "MyGameList",
    environment: "Development",
    updated: "Today",
    created: "August 12, 2026",
    provider: "OpenAI",
    value: "sk-proj-71ndKw02mfPzQ4",
  },
  {
    id: "s11",
    name: "Home Server",
    type: "SSH",
    project: "Infrastructure",
    environment: "—",
    updated: "2 months ago",
    created: "January 9, 2026",
    value: "-----BEGIN OPENSSH PRIVATE KEY-----",
  },
  {
    id: "s12",
    name: "Recovery Codes",
    type: "Note",
    project: "Personal",
    environment: "—",
    updated: "1 month ago",
    created: "February 21, 2026",
    notes: "Backup codes for the org account",
    value: "8291-4410 · 7723-9014 · 1182-6650",
  },
  {
    id: "s13",
    name: "REDIS_URL",
    type: "Database",
    project: "Rescripto",
    environment: "Staging",
    updated: "8 days ago",
    created: "May 27, 2026",
    value: "redis://cache.internal:6379",
    damaged: true,
  },
];

export const projects: Project[] = [
  {
    id: "rescripto",
    name: "Rescripto",
    environments: [
      { name: "Development", count: 8 },
      { name: "Staging", count: 2 },
      { name: "Production", count: 5 },
    ],
  },
  {
    id: "namevetta",
    name: "NameVetta",
    environments: [
      { name: "Development", count: 6 },
      { name: "Production", count: 4 },
    ],
  },
  {
    id: "mygamelist",
    name: "MyGameList",
    environments: [
      { name: "Development", count: 9 },
      { name: "Production", count: 3 },
    ],
  },
  {
    id: "infrastructure",
    name: "Infrastructure",
    environments: [
      { name: "Production", count: 3 },
      { name: "Staging", count: 1 },
    ],
  },
];

export const devices: Device[] = [
  {
    id: "d1",
    name: "Android Phone",
    status: "Trusted",
    lastSync: "2 min ago",
    fingerprint: "3F:82:91:A4:27:D0:5B:11",
    deviceId: "ENV-A39F2C81",
    added: "August 20, 2026",
  },
  {
    id: "d2",
    name: "Development Laptop",
    status: "Trusted",
    lastSync: "Yesterday",
    fingerprint: "A1:04:7C:9E:63:B8:2F:40",
    deviceId: "ENV-77C1B004",
    added: "June 2, 2026",
  },
  {
    id: "d3",
    name: "Work Desktop",
    status: "Offline",
    lastSync: "Yesterday",
    fingerprint: "C9:5D:18:33:AB:70:E2:64",
    deviceId: "ENV-2B90DA55",
    added: "March 15, 2026",
  },
];

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
    label: "API & Tokens",
    types: ["API Key", "Token", "OAuth", "Webhook"] as SecretType[],
  },
  databases: { label: "Databases", types: ["Database"] as SecretType[] },
  ssh: { label: "SSH", types: ["SSH"] as SecretType[] },
  notes: { label: "Secure Notes", types: ["Note"] as SecretType[] },
};
