import * as React from "react";
import type { Environment, Secret } from "@/lib/envryn-data";

/** Prefill for the .env import modal when opened from a project context. */
export interface ImportPreset {
  project?: string;
  environment?: Environment;
}

interface VaultUI {
  selected: Secret | null;
  select: (s: Secret | null) => void;
  openAdd: (preset?: Partial<Secret>) => void;
  openEdit: (s: Secret) => void;
  openSearch: () => void;
  openImport: (preset?: ImportPreset) => void;
  openExtract: () => void;
}

export const VaultUIContext = React.createContext<VaultUI>({
  selected: null,
  select: () => {},
  openAdd: () => {},
  openEdit: () => {},
  openSearch: () => {},
  openImport: () => {},
  openExtract: () => {},
});

export const useVaultUI = () => React.useContext(VaultUIContext);
