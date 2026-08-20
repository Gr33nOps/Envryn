import * as React from "react";
import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { Fingerprint, Lock, ShieldCheck } from "lucide-react";
import { MobileInput, TouchButton } from "@/components/envryn/mobile/Sheet";

export const Route = createFileRoute("/m/")({
  head: () => ({
    meta: [
      { title: "Envryn Mobile — Unlock your vault" },
      {
        name: "description",
        content:
          "Unlock the Envryn mobile vault with biometrics or your master password to access API keys, tokens and credentials on the go.",
      },
      { property: "og:title", content: "Envryn Mobile — Unlock your vault" },
      {
        property: "og:description",
        content: "Local-first developer secrets vault, now on your phone.",
      },
      { property: "og:type", content: "website" },
      { name: "twitter:card", content: "summary_large_image" },
    ],
  }),
  component: MobileUnlock,
});

function MobileUnlock() {
  const navigate = useNavigate();
  const [pw, setPw] = React.useState("");
  const [error, setError] = React.useState(false);

  const unlock = () => navigate({ to: "/m/vault" });

  return (
    <div className="flex h-full flex-col justify-between px-6 pb-8 pt-16">
      <div className="flex flex-1 flex-col items-center justify-center">
        <div className="grid size-14 place-items-center rounded-2xl border border-border bg-surface">
          <Lock className="size-6 text-primary" />
        </div>
        <h1 className="mt-5 text-[20px] font-semibold tracking-[-0.02em]">Envryn</h1>
        <p className="mt-1 text-[12.5px] text-muted-foreground">
          Vault locked · 26 secrets on this device
        </p>

        <button
          onClick={unlock}
          className="mt-10 grid size-24 place-items-center rounded-full border border-primary/35 bg-primary-muted active:scale-95"
          aria-label="Unlock with biometrics"
        >
          <Fingerprint className="size-11 text-primary" />
        </button>
        <p className="mt-3 text-[12.5px] text-muted-foreground">
          Touch to unlock with biometrics
        </p>
      </div>

      <div className="space-y-3">
        <div className="flex items-center gap-3 text-[11px] text-subtle-foreground">
          <span className="h-px flex-1 bg-border" />
          or master password
          <span className="h-px flex-1 bg-border" />
        </div>
        <MobileInput
          type="password"
          value={pw}
          onChange={(e) => {
            setPw(e.target.value);
            setError(false);
          }}
          placeholder="Master password"
        />
        {error && (
          <p className="text-[12px] text-destructive">
            Incorrect password. 4 attempts remaining.
          </p>
        )}
        <TouchButton
          variant="primary"
          className="w-full"
          onClick={() => (pw.length >= 4 ? unlock() : setError(true))}
        >
          Unlock vault
        </TouchButton>
        <p className="flex items-center justify-center gap-1.5 text-[11.5px] text-subtle-foreground">
          <ShieldCheck className="size-3.5" />
          Encrypted locally · No cloud account
        </p>
      </div>
    </div>
  );
}
