import * as React from "react";
import { createFileRoute } from "@tanstack/react-router";
import { ArrowDownUp, ChevronDown, Plus, Sparkles, Upload } from "lucide-react";
import { useSecretList } from "@/lib/use-vault";
import { SecretList } from "@/components/envryn/SecretList";
import { useVaultUI } from "@/components/envryn/vault-context";
import { Button, EmptyState, Panel, SearchField, Tabs } from "@/components/envryn/ui";

export const Route = createFileRoute("/vault/")({
  component: AllSecrets,
});

const filters = [
  { value: "all", label: "All" },
  { value: "API Key", label: "API keys" },
  { value: "Token", label: "Tokens" },
  { value: "Database", label: "Databases" },
  { value: "SSH", label: "SSH" },
];

function AllSecrets() {
  const { openAdd, openImport, openExtract } = useVaultUI();
  const secrets = useSecretList();
  const [filter, setFilter] = React.useState("all");
  const [query, setQuery] = React.useState("");
  const [environment, setEnvironment] = React.useState("all");
  const [sortNewest, setSortNewest] = React.useState(true);

  const items = React.useMemo(() => {
    const normalized = query.trim().toLowerCase();
    return secrets
      .filter((secret) => filter === "all" || secret.type === filter)
      .filter((secret) => environment === "all" || secret.environment === environment)
      .filter((secret) =>
        [secret.name, secret.project, secret.environment, secret.type, secret.provider ?? ""]
          .join(" ")
          .toLowerCase()
          .includes(normalized),
      )
      .sort((a, b) => (sortNewest ? b.id.localeCompare(a.id) : a.id.localeCompare(b.id)));
  }, [environment, filter, query, secrets, sortNewest]);

  return (
    <div className="min-h-full bg-background">
      <div className="content-wrap content-wrap--narrow">
        <header className="page-hero">
          <div>
            <p className="breadcrumb">
              Vault <span>/</span> All secrets
            </p>
            <div className="mt-3 flex items-center gap-2.5">
              <h1 className="text-[22px] font-semibold tracking-[-0.035em] text-foreground">
                Secrets
              </h1>
              <span className="count-pill">{secrets.length}</span>
            </div>
            <p className="mt-1.5 max-w-[58ch] text-[12.5px] leading-relaxed text-muted-foreground">
              Your local development credentials, organized by project and environment.
            </p>
          </div>
          <div className="flex shrink-0 items-center gap-2">
            <Button variant="primary" size="lg" onClick={() => openAdd()}>
              <Plus />
              Add secret
            </Button>
          </div>
        </header>

        <div className="mb-3 flex flex-wrap items-center justify-between gap-2 border-y border-border/70 py-2.5">
          <span className="text-[11.5px] text-muted-foreground">
            {items.length} of {secrets.length} secrets · Select a row to view or copy its value.
          </span>
          <span className="text-[11.5px] text-subtle-foreground">Changes save automatically</span>
        </div>

        <Panel className="overflow-visible">
          <div className="flex flex-col gap-2.5 border-b border-border px-3.5 py-3 md:flex-row md:items-center">
            <SearchField
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder="Search by name, project, or provider"
              className="min-w-0 flex-1 md:max-w-[390px]"
            />
            <div className="flex items-center gap-2 md:ml-auto">
              <label className="relative flex items-center">
                <select
                  aria-label="Filter by environment"
                  value={environment}
                  onChange={(event) => setEnvironment(event.target.value)}
                  className="filter-select"
                >
                  <option value="all">All environments</option>
                  <option value="Development">Development</option>
                  <option value="Staging">Staging</option>
                  <option value="Production">Production</option>
                </select>
                <ChevronDown className="pointer-events-none absolute right-2 size-3.5 text-subtle-foreground" />
              </label>
              <Button
                variant="secondary"
                size="md"
                onClick={() => setSortNewest((value) => !value)}
              >
                <ArrowDownUp />
                {sortNewest ? "Newest" : "Oldest"}
              </Button>
            </div>
          </div>
          <Tabs
            items={filters}
            value={filter}
            onChange={setFilter}
            className="overflow-x-auto px-3.5"
          />
          {items.length === 0 ? (
            <EmptyState
              title={query ? `No results for “${query}”` : "No secrets in this view"}
              body="Try a different filter or environment."
              action={
                query ? (
                  <Button variant="secondary" onClick={() => setQuery("")}>
                    Clear search
                  </Button>
                ) : (
                  <Button variant="primary" onClick={() => openAdd()}>
                    <Plus /> Add secret
                  </Button>
                )
              }
            />
          ) : (
            <SecretList items={items} />
          )}
          <div className="flex items-center justify-between border-t border-border bg-background/35 px-3.5 py-2 text-[10.5px] text-subtle-foreground">
            <span>Encrypted values stay on this device.</span>
            <div className="flex items-center gap-3">
              <button
                type="button"
                className="inline-flex items-center gap-1 text-primary transition-colors hover:text-foreground"
                onClick={() => openExtract()}
              >
                <Sparkles className="size-3" />
                Extract fields
              </button>
              <button
                type="button"
                className="inline-flex items-center gap-1 text-primary transition-colors hover:text-foreground"
                onClick={() => openImport()}
              >
                <Upload className="size-3" />
                Import .env
              </button>
            </div>
          </div>
        </Panel>
      </div>
    </div>
  );
}
