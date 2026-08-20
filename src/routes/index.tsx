import * as React from "react";
import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { Button, Input } from "@/components/envryn/ui";

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
    <main className="flex h-screen items-center justify-center bg-background">
      <form onSubmit={submit} className="w-[248px] text-center">
        <h1 className="text-[15px] font-semibold tracking-[-0.01em]">Envryn</h1>
        <p className="mt-1 text-[12.5px] text-muted-foreground">Vault is locked</p>

        <div className="mt-6 space-y-2 text-left">
          <Input
            autoFocus
            type="password"
            placeholder="Master password"
            className="h-8 text-center"
            invalid={error}
            value={value}
            onChange={(e) => {
              setValue(e.target.value);
              setError(false);
            }}
          />
          {error && (
            <p className="text-center text-[11.5px] text-destructive">
              Incorrect password. Try again.
            </p>
          )}
          <Button type="submit" variant="primary" size="block" loading={loading}>
            Unlock Vault
          </Button>
          <Button type="button" variant="ghost" size="block">
            Use Windows Hello
          </Button>
        </div>

        <p className="mt-6 text-[11px] text-subtle-foreground">
          Your vault stays encrypted until you unlock it.
        </p>
      </form>
    </main>
  );
}
