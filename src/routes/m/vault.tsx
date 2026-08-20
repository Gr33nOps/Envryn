import * as React from "react";
import { createFileRoute } from "@tanstack/react-router";
import { Plus, Search, Lock, Filter } from "lucide-react";
import { toast } from "sonner";
import { useNavigate } from "@tanstack/react-router";
import { cn } from "@/lib/utils";
import { secrets } from "@/lib/envryn-data";
import { HeaderIcon, MobileHeader } from "@/components/envryn/mobile/Shell";
import { SecretRow } from "@/components/envryn/mobile/SecretRow";
import { ListCard, TouchButton } from "@/components/envryn/mobile/Sheet";
import { useMobileUI } from "@/components/envryn/mobile/mobile-context";

export const Route = createFileRoute("/m/vault")({
  head: () => ({
    meta: [
      { title: "Mobile Vault — Envryn" },
      {
        name: "description",
        content:
          "Browse, search, reveal and copy your API keys, tokens and credentials from the Envryn mobile vault.",
      },
      { property: "og:title", content: "Mobile Vault — Envryn" },
      {
        property: "og:description",
        content: "All your developer secrets, organized by project and environment.",
      },
      { property: "og:type", content: "website" },
      { name: "twitter:card", content: "summary_large_image" },
    ],
  }),
  component: MobileVault,
});

const filters = ["All", "Recent", "Production", "API Key", "Database", "SSH"];

function MobileVault() {
  const { openAdd, openSearch } = useMobileUI();
  const navigate = useNavigate();
  const [filter, setFilter] = React.useState("All");

  const items = secrets.filter((s) =>
    filter === "All"
      ? true
      : filter === "Recent"
        ? /day|Yesterday|hour/i.test(s.updated)
        : filter === "Production"
          ? s.environment === "Production"
          : s.type === filter,
  );

  return (
    <div className="pb-6">
      <MobileHeader
        title="All Secrets"
        subtitle={`${secrets.length} secrets · unlocked`}
        right={
          <>
            <HeaderIcon label="Search" onClick={openSearch}>
              <Search />
            </HeaderIcon>
            <HeaderIcon
              label="Lock vault"
              onClick={() => {
                toast("Vault locked");
                navigate({ to: "/m" });
              }}
            >
              <Lock />
            </HeaderIcon>
          </>
        }
      />

      <div className="flex gap-1.5 overflow-x-auto px-3 py-2.5 [scrollbar-width:none]">
        <span className="grid size-8 shrink-0 place-items-center rounded-lg border border-border bg-surface text-subtle-foreground">
          <Filter className="size-3.5" />
        </span>
        {filters.map((f) => (
          <button
            key={f}
            onClick={() => setFilter(f)}
            className={cn(
              "h-8 shrink-0 rounded-lg border px-3 text-[12.5px] transition-colors",
              f === filter
                ? "border-primary bg-primary-muted text-foreground"
                : "border-border bg-surface text-muted-foreground",
            )}
          >
            {f}
          </button>
        ))}
      </div>

      <div className="px-3">
        {items.length === 0 ? (
          <div className="rounded-xl border border-dashed border-border px-4 py-14 text-center">
            <p className="text-[13.5px]">No secrets here yet</p>
            <p className="mt-1 text-[12px] text-muted-foreground">
              Add your first {filter !== "All" ? filter.toLowerCase() : ""} secret to
              this vault.
            </p>
            <TouchButton
              variant="primary"
              className="mx-auto mt-4"
              onClick={() => openAdd()}
            >
              <Plus />
              Add secret
            </TouchButton>
          </div>
        ) : (
          <ListCard>
            {items.map((s) => (
              <SecretRow key={s.id} secret={s} />
            ))}
          </ListCard>
        )}
      </div>

      <button
        onClick={() => openAdd()}
        aria-label="Add secret"
        className="fixed bottom-24 left-1/2 z-30 ml-[130px] grid size-13 size-14 place-items-center rounded-2xl bg-primary text-primary-foreground shadow-lg shadow-black/40 active:scale-95"
      >
        <Plus className="size-6" />
      </button>
    </div>
  );
}
