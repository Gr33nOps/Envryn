import * as React from "react";
import { createFileRoute, notFound } from "@tanstack/react-router";
import { Plus } from "lucide-react";
import { categories } from "@/lib/envryn-data";
import { useSecretList } from "@/lib/use-vault";
import { SecretList } from "@/components/envryn/SecretList";
import { useVaultUI } from "@/components/envryn/vault-context";
import { Button, EmptyState, SearchField } from "@/components/envryn/ui";

export const Route = createFileRoute("/vault/category/$categoryId")({
  loader: ({ params }) => {
    const category = categories[params.categoryId as keyof typeof categories];
    if (!category) throw notFound();
    return category;
  },
  component: CategoryView,
});

function CategoryView() {
  const secrets = useSecretList();
  const category = Route.useLoaderData();
  const { openAdd } = useVaultUI();
  const [query, setQuery] = React.useState("");
  const allItems = secrets.filter((secret) => category.types.includes(secret.type));
  const normalizedQuery = query.trim().toLowerCase();
  const items = allItems.filter((secret) =>
    [secret.name, secret.project, secret.type, secret.provider ?? ""]
      .join(" ")
      .toLowerCase()
      .includes(normalizedQuery),
  );

  return (
    <div className="min-h-full bg-background">
      <div className="content-wrap content-wrap--narrow">
        <header className="page-hero">
          <div>
            <p className="breadcrumb">
              Vault <span>/</span> {category.label}
            </p>
            <h1 className="mt-3 text-[22px] font-semibold tracking-[-0.035em]">{category.label}</h1>
            <p className="mt-1.5 max-w-[62ch] text-[12.5px] text-muted-foreground">
              {category.description}{" "}
              <span className="text-subtle-foreground">
                {allItems.length} {allItems.length === 1 ? "secret" : "secrets"} saved here.
              </span>
            </p>
          </div>
          <Button variant="primary" size="lg" onClick={() => openAdd()}>
            <Plus />
            Add secret
          </Button>
        </header>
        {allItems.length > 0 && (
          <div className="mb-3 flex flex-wrap items-center justify-between gap-2">
            <SearchField
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder={`Search ${category.label.toLowerCase()}...`}
              className="max-w-[360px]"
            />
            <span className="text-[11.5px] text-subtle-foreground">{items.length} shown</span>
          </div>
        )}
        {items.length === 0 ? (
          <EmptyState
            title={query ? `No results for “${query}”` : `Nothing in ${category.label}`}
            body={
              query
                ? "Try a different name, project, or provider."
                : "Add your first secret in this category to see it here."
            }
            action={
              query ? (
                <Button variant="secondary" onClick={() => setQuery("")}>
                  Clear search
                </Button>
              ) : (
                <Button variant="primary" onClick={() => openAdd()}>
                  <Plus />
                  Add secret
                </Button>
              )
            }
          />
        ) : (
          <div className="overflow-hidden rounded-lg border border-border bg-surface">
            <SecretList items={items} />
          </div>
        )}
      </div>
    </div>
  );
}
