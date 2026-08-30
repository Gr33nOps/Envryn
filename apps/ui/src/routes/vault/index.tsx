import * as React from "react";
import { createFileRoute } from "@tanstack/react-router";
import { ArrowDownUp, ChevronDown, Plus, Sparkles, Upload } from "lucide-react";
import { categories, type SecretType } from "@/lib/envryn-data";
import { useSecretList } from "@/lib/use-vault";
import { SecretList } from "@/components/envryn/SecretList";
import { useVaultUI } from "@/components/envryn/vault-context";
import { Button, EmptyState, Panel, SearchField, Tabs } from "@/components/envryn/ui";

export const Route = createFileRoute("/vault/")({
  component: AllSecrets,
});

// Mirrors the sidebar's own category grouping (`envryn-data.ts`'s
// `categories`) rather than listing individual secret types -- these used to
// disagree (the sidebar showed one combined "API & tokens" entry while this
// page split it into separate "API keys"/"Tokens" tabs), which read as two
// different category schemes for the same vault. One source of truth now.
const filters = [
  { value: "all", label: "All" },
  ...Object.entries(categories).map(([value, category]) => ({
    value,
    label: category.label,
  })),
];

function matchesFilter(secretType: SecretType, filter: string): boolean {
  if (filter === "all") return true;
  const category = categories[filter as keyof typeof categories];
  return category ? (category.types as SecretType[]).includes(secretType) : false;
}

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
      .filter((secret) => matchesFilter(secret.type, filter))
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
          {/* The empty state below already offers its own centered "Add secret" --
              showing this one too, on a screen with nothing else on it, was two
              identical calls to action competing for the same click. */}
          {secrets.length > 0 && (
            <div className="flex shrink-0 items-center gap-2">
              <Button variant="primary" size="lg" onClick={() => openAdd()}>
                <Plus />
                Add secret
              </Button>
            </div>
          )}
        </header>

        <div className="mb-3 flex flex-wrap items-center justify-between gap-2 border-y border-border/70 py-2.5">
          <span className="text-[11.5px] text-muted-foreground">
            {items.length} of {secrets.length} secrets · Select a row to view or copy its value.
          </span>
          <span className="text-[11.5px] text-subtle-foreground">Changes save automatically</span>
        </div>

        <Panel className="overflow-visible">
          <div className="secret-controls flex flex-col gap-2.5 border-b border-border px-3.5 py-3 md:flex-row md:items-center">
            <SearchField
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder="Filter this list by name, project, or provider"
              className="min-w-0 flex-1 md:max-w-[390px]"
            />
            <div className="secret-filter-row flex items-center gap-2 md:ml-auto">
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
            className="desktop-category-tabs overflow-x-auto px-3.5"
          />
          <label className="mobile-category-filter">
            <span>Secret category</span>
            <div className="relative">
              <select
                aria-label="Filter by secret category"
                value={filter}
                onChange={(event) => setFilter(event.target.value)}
                className="filter-select"
              >
                {filters.map((item) => (
                  <option key={item.value} value={item.value}>
                    {item.label}
                  </option>
                ))}
              </select>
              <ChevronDown className="pointer-events-none absolute right-3 top-1/2 size-4 -translate-y-1/2 text-subtle-foreground" />
            </div>
          </label>
          {items.length === 0 ? (
            secrets.length === 0 ? (
              <EmptyState
                title="No secrets yet"
                body="Add your first secret to get started."
                action={
                  <Button variant="primary" onClick={() => openAdd()}>
                    <Plus /> Add secret
                  </Button>
                }
              />
            ) : query ? (
              <EmptyState
                title={`No results for “${query}”`}
                body="Try a different name, project, or provider."
                action={
                  <Button variant="secondary" onClick={() => setQuery("")}>
                    Clear search
                  </Button>
                }
              />
            ) : (
              <EmptyState
                title="No secrets match these filters"
                body="Try a different filter or environment."
                action={
                  <Button
                    variant="secondary"
                    onClick={() => {
                      setFilter("all");
                      setEnvironment("all");
                    }}
                  >
                    Clear filters
                  </Button>
                }
              />
            )
          ) : (
            <SecretList items={items} />
          )}
          <div className="vault-list-footer flex items-center justify-between border-t border-border bg-background/35 px-3.5 py-2 text-[10.5px] text-subtle-foreground">
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
