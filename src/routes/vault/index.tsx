import * as React from "react";
import { createFileRoute } from "@tanstack/react-router";
import { Plus } from "lucide-react";
import { secrets } from "@/lib/envryn-data";
import { SecretList } from "@/components/envryn/SecretList";
import { useVaultUI } from "@/components/envryn/vault-context";
import {
  Button,
  EmptyState,
  SearchField,
  Select,
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

const projects = Array.from(new Set(secrets.map((s) => s.project))).sort();
const environments = ["Development", "Staging", "Production"];

function AllSecrets() {
  const { openAdd } = useVaultUI();
  const [filter, setFilter] = React.useState("all");
  const [q, setQ] = React.useState("");
  const [project, setProject] = React.useState("all");
  const [env, setEnv] = React.useState("all");

  const items = secrets.filter(
    (s) =>
      (filter === "all" || s.type === filter) &&
      (project === "all" || s.project === project) &&
      (env === "all" || s.environment === env) &&
      [s.name, s.project, s.environment, s.type, s.provider ?? ""]
        .join(" ")
        .toLowerCase()
        .includes(q.trim().toLowerCase()),
  );

  const filtered = filter !== "all" || project !== "all" || env !== "all" || q;

  return (
    <div className="flex h-full min-h-0 flex-col px-5 pb-5 pt-4">
      {/* Header — title, context, primary action on one grid line */}
      <div className="flex shrink-0 items-center justify-between gap-4">
        <div className="min-w-0">
          <h1 className="truncate text-[15px] font-semibold tracking-[-0.01em]">
            All Secrets
          </h1>
          <p className="mt-0.5 text-[12px] text-muted-foreground">
            {filtered
              ? `${items.length} of ${secrets.length} secrets`
              : `${secrets.length} secrets`}{" "}
            · {projects.length} projects
          </p>
        </div>
        <Button variant="primary" onClick={() => openAdd()}>
          <Plus />
          Add Secret
        </Button>
      </div>

      {/* Toolbar — search dominates, filters share the same row */}
      <div className="mt-3 flex shrink-0 items-center gap-2">
        <SearchField
          value={q}
          onChange={(e) => setQ(e.target.value)}
          placeholder="Search secrets, projects, providers..."
          shortcut="Ctrl K"
          className="min-w-0 flex-1"
        />
        <Select
          value={project}
          onChange={(e) => setProject(e.target.value)}
          className="w-[136px]"
        >
          <option value="all">All projects</option>
          {projects.map((p) => (
            <option key={p} value={p}>
              {p}
            </option>
          ))}
        </Select>
        <Select
          value={env}
          onChange={(e) => setEnv(e.target.value)}
          className="w-[136px]"
        >
          <option value="all">All environments</option>
          {environments.map((e) => (
            <option key={e} value={e}>
              {e}
            </option>
          ))}
        </Select>
      </div>

      <Tabs
        items={filters}
        value={filter}
        onChange={setFilter}
        className="mt-2.5 shrink-0"
      />

      <div className="mt-2.5 flex min-h-0 flex-1 flex-col">
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
          <SecretList items={items} fill />
        )}
      </div>
    </div>
  );
}
