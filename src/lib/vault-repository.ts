import { devices, projects, secrets, type Device, type Project, type Secret } from "./envryn-data";

export type CreateSecretInput = Omit<Secret, "id" | "created" | "updated">;
export type UpdateSecretInput = Partial<CreateSecretInput>;

/**
 * Persistence contract for the vault UI.
 *
 * The first implementation is intentionally local mock data. A desktop
 * adapter can later implement the same contract with an encrypted SQLite
 * store or the Tauri/Electron bridge without changing page components.
 */
export interface VaultRepository {
  listSecrets(): Promise<Secret[]>;
  listProjects(): Promise<Project[]>;
  listDevices(): Promise<Device[]>;
  createSecret(input: CreateSecretInput): Promise<Secret>;
  updateSecret(id: string, input: UpdateSecretInput): Promise<Secret>;
  deleteSecret(id: string): Promise<void>;
}

const copy = <T>(value: T): T => structuredClone(value);

export const mockVaultRepository: VaultRepository = {
  async listSecrets() {
    return copy(secrets);
  },
  async listProjects() {
    return copy(projects);
  },
  async listDevices() {
    return copy(devices);
  },
  async createSecret(input) {
    const created: Secret = {
      ...input,
      id: `local-${Date.now()}`,
      created: "Just now",
      updated: "Just now",
    };
    return copy(created);
  },
  async updateSecret(id, input) {
    const current = secrets.find((secret) => secret.id === id);
    if (!current) throw new Error(`Secret ${id} was not found`);
    return copy({ ...current, ...input, updated: "Just now" });
  },
  async deleteSecret(id) {
    if (!secrets.some((secret) => secret.id === id)) throw new Error(`Secret ${id} was not found`);
  },
};
