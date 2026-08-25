/**
 * Live vault data.
 *
 * One query key for the record list; everything else (projects, environment
 * counts, category membership) is derived from it. Deriving rather than
 * fetching separately means the sidebar counts cannot disagree with the list
 * they describe.
 */
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import * as React from "react";

import { environmentOrder, type Environment, type Project, type Secret } from "./envryn-data";
import {
  tauriVaultRepository,
  type CreateSecretInput,
  type UpdateSecretInput,
} from "./vault-repository";

const SECRETS_KEY = ["secrets"] as const;
const DEVICES_KEY = ["devices"] as const;

export function useSecrets() {
  return useQuery({
    queryKey: SECRETS_KEY,
    queryFn: () => tauriVaultRepository.listSecrets(),
  });
}

/** The record list, or an empty array while loading. For rendering only. */
export function useSecretList(): Secret[] {
  const { data } = useSecrets();
  return data ?? [];
}

export function useDevices() {
  return useQuery({
    queryKey: DEVICES_KEY,
    queryFn: () => tauriVaultRepository.listDevices(),
  });
}

export function useRenameDevice() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: ({ deviceId, name }: { deviceId: string; name: string }) =>
      tauriVaultRepository.renameDevice(deviceId, name),
    onSuccess: () => client.invalidateQueries({ queryKey: DEVICES_KEY }),
  });
}

export function useRevokeDevice() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (deviceId: string) => tauriVaultRepository.revokeDevice(deviceId),
    onSuccess: () => client.invalidateQueries({ queryKey: DEVICES_KEY }),
  });
}

/**
 * Projects, derived from the records that reference them.
 *
 * There is no separate project table: a project exists exactly as long as
 * something is filed under it. That avoids a whole class of drift where the
 * sidebar lists a project the vault no longer contains.
 */
export function useProjects(): Project[] {
  const secrets = useSecretList();

  return React.useMemo(() => {
    const byProject = new Map<string, Map<Environment, number>>();

    for (const secret of secrets) {
      const environments = byProject.get(secret.project) ?? new Map();
      environments.set(secret.environment, (environments.get(secret.environment) ?? 0) + 1);
      byProject.set(secret.project, environments);
    }

    return [...byProject.entries()]
      .map(([name, environments]) => ({
        id: name.toLowerCase().replace(/[^a-z0-9]+/g, "-"),
        name,
        environments: environmentOrder
          .filter((env) => environments.has(env))
          .map((env) => ({ name: env, count: environments.get(env) ?? 0 })),
      }))
      .sort((a, b) => a.name.localeCompare(b.name));
  }, [secrets]);
}

export function useCreateSecret() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (input: CreateSecretInput) => tauriVaultRepository.createSecret(input),
    onSuccess: () => client.invalidateQueries({ queryKey: SECRETS_KEY }),
  });
}

export function useUpdateSecret() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: ({ id, input }: { id: string; input: UpdateSecretInput }) =>
      tauriVaultRepository.updateSecret(id, input),
    onSuccess: () => client.invalidateQueries({ queryKey: SECRETS_KEY }),
  });
}

export function useDeleteSecret() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => tauriVaultRepository.deleteSecret(id),
    onSuccess: () => client.invalidateQueries({ queryKey: SECRETS_KEY }),
  });
}

/**
 * Reveal a single secret value.
 *
 * Deliberately not a query: revealing is an action the user takes, not state
 * to be cached. Caching it would keep plaintext in memory after the panel that
 * asked for it had closed.
 */
export function useRevealSecret() {
  return useMutation({
    mutationFn: (id: string) => tauriVaultRepository.revealSecret(id),
  });
}

/** Drop every cached record. Called on lock so nothing survives in the cache. */
export function useClearVaultCache() {
  const client = useQueryClient();
  return React.useCallback(() => {
    client.removeQueries({ queryKey: SECRETS_KEY });
    client.removeQueries({ queryKey: DEVICES_KEY });
  }, [client]);
}

/**
 * Refetch rather than clear: after a backup restore the vault is already
 * unlocked again, holding a different set of records under a different
 * password than whatever was cached a moment ago.
 */
export function useRefreshVaultCache() {
  const client = useQueryClient();
  return React.useCallback(() => {
    void client.invalidateQueries({ queryKey: SECRETS_KEY });
  }, [client]);
}
