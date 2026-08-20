import * as React from "react";
import type { Secret } from "@/lib/envryn-data";

interface MobileUI {
  selected: Secret | null;
  select: (s: Secret | null) => void;
  openAdd: (preset?: Partial<Secret>) => void;
  openEdit: (s: Secret) => void;
  openSearch: () => void;
}

export const MobileUIContext = React.createContext<MobileUI>({
  selected: null,
  select: () => {},
  openAdd: () => {},
  openEdit: () => {},
  openSearch: () => {},
});

export const useMobileUI = () => React.useContext(MobileUIContext);
