import * as React from "react";
import { createFileRoute } from "@tanstack/react-router";
import { Plus } from "lucide-react";
import { secrets } from "@/lib/envryn-data";
import { SecretList } from "@/components/envryn/SecretList";
import { useVaultUI } from "@/components/envryn/vault-context";
import {
  Button,
  EmptyState,
  PageHeader,
  SearchField,
  Tabs,
} from "@/components/envryn/ui";

export const Route = createFileRoute("/vault/")({
  component: AllSecrets,
});

const filters = [
  { value: "all", label: "All" },
  { value: "API Key", label: "API Keys" },
  { value: "Token", label: "Tokens" },
  { value: "Database", label: "Databases" },
  { value: "SSH", label: "SSH" },
];

function AllSecrets() {
  const { openAdd } = useVaultUI();
  const [filter, setFilter] = React.useState("all");
  const [q, setQ] = React.useState("");

  const items = secrets.filter(
    (s) =>
      (filter === "all" || s.type === filter) &&
      [s.name, s.project, s.environment, s.type, s.provider ?? ""]
        .join(" ")
        .toLowerCase()
        .includes(q.trim().toLowerCase()),
  );

  return (
    <>
      <PageHeader
        title="All Secrets"
        actions={
          <Button variant="primary" onClick={() => openAdd()}>
            <Plus />
            Add Secret
          </Button>
        }
      />
      <div className="space-y-2.5 px-5 pb-5">
        <SearchField
          value={q}
          onChange={(e) => setQ(e.target.value)}
          placeholder="Search secrets..."
          shortcut="Ctrl K"
          className="max-w-[320px]"
        />
        <Tabs items={filters} value={filter} onChange={setFilter} />

        {items.length === 0 ? (
          q ? (
            <EmptyState
              title={`No results for "${q}"`}
              body="Try another name, project, or tag."
            />
          ) : (
            <EmptyState
              title="No secrets yet"
              body="Store your first API key, token, database credential, or other development secret."
              action={
                <Button variant="primary" onClick={() => openAdd()}>
                  <Plus />
                  Add Secret
                </Button>
              }
            />
          )
        ) : (
          <SecretList items={items} />
        )}
      </div>
    </>
  );
}
