import * as React from "react";
import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { Button, Input } from "@/components/envryn/ui";
import { LogoMark } from "@/components/envryn/Logo";

export const Route = createFileRoute("/")({
  head: () => ({
    meta: [
      { title: "Envryn — Unlock your developer secrets vault" },
      {
        name: "description",
        content:
          "Envryn is a local-first developer secrets vault for API keys, tokens, database and SSH credentials. Unlock to access your vault.",
      },
      { property: "og:title", content: "Envryn — Developer secrets vault" },
      {
        property: "og:description",
        content:
          "Local-first vault for API keys, tokens, database and SSH credentials.",
      },
      { property: "og:type", content: "website" },
      { name: "twitter:card", content: "summary_large_image" },
    ],
  }),
  component: Unlock,
});

function Unlock() {
  const navigate = useNavigate();
  const [value, setValue] = React.useState("");
  const [error, setError] = React.useState(false);
  const [loading, setLoading] = React.useState(false);

  function submit(e: React.FormEvent) {
    e.preventDefault();
    if (value.trim().toLowerCase() === "wrong") return setError(true);
    setLoading(true);
    setTimeout(() => navigate({ to: "/vault" }), 400);
  }

  return (
    <main className="relative flex h-screen items-center justify-center bg-background">
      <div className="pointer-events-none absolute inset-x-0 top-0 h-px bg-gradient-to-r from-transparent via-[var(--brand-border)] to-transparent" />
      <form onSubmit={submit} className="w-[268px]">
        <div className="flex flex-col items-center">
          <LogoMark size={34} />
          <h1 className="mt-3 text-[14px] font-semibold tracking-[-0.015em]">
            Envryn
          </h1>
          <p className="mt-1 font-mono text-[11px] tracking-tight text-subtle-foreground">
            vault locked · local-only
          </p>
        </div>

        <div className="mt-7 space-y-2">
          <Input
            autoFocus
            type="password"
            placeholder="Master password"
            className="h-8"
            invalid={error}
            value={value}
            onChange={(e) => {
              setValue(e.target.value);
              setError(false);
            }}
          />
          {error && (
            <p className="text-[11.5px] text-destructive">
              Incorrect password. Try again.
            </p>
          )}
          <Button type="submit" variant="primary" size="block" loading={loading}>
            Unlock Vault
          </Button>
          <Button type="button" variant="secondary" size="block">
            Use Windows Hello
          </Button>
        </div>

        <div className="mt-7 flex items-center justify-between border-t border-border pt-2.5 text-[11px] text-subtle-foreground">
          <span>Encrypted at rest</span>
          <span className="font-mono tracking-tight">AES-256-GCM</span>
        </div>
      </form>
    </main>

  );
}
