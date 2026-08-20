import * as React from "react";
import type { Secret } from "@/lib/envryn-data";

interface VaultUI {
  selected: Secret | null;
  select: (s: Secret | null) => void;
  openAdd: (preset?: Partial<Secret>) => void;
  openEdit: (s: Secret) => void;
  openSearch: () => void;
}

export const VaultUIContext = React.createContext<VaultUI>({
  selected: null,
  select: () => {},
  openAdd: () => {},
  openEdit: () => {},
  openSearch: () => {},
});

export const useVaultUI = () => React.useContext(VaultUIContext);
