import { createFileRoute, Link } from "@tanstack/react-router";
import { ChevronRight, FolderClosed, Search } from "lucide-react";
import { projects } from "@/lib/envryn-data";
import { HeaderIcon, MobileHeader } from "@/components/envryn/mobile/Shell";
import { ListCard } from "@/components/envryn/mobile/Sheet";
import { EnvDot } from "@/components/envryn/mobile/SecretRow";
import { useMobileUI } from "@/components/envryn/mobile/mobile-context";

export const Route = createFileRoute("/m/projects/")({
  head: () => ({
    meta: [
      { title: "Projects — Envryn Mobile" },
      {
        name: "description",
        content:
          "Your secrets grouped by project and by development, staging and production environments.",
      },
      { property: "og:title", content: "Projects — Envryn Mobile" },
      {
        property: "og:description",
        content: "Secrets grouped by project and environment.",
      },
      { property: "og:type", content: "website" },
      { name: "twitter:card", content: "summary_large_image" },
    ],
  }),
  component: MobileProjects,
});

function MobileProjects() {
  const { openSearch } = useMobileUI();
  return (
    <div className="pb-6">
      <MobileHeader
        title="Projects"
        subtitle={`${projects.length} projects`}
        right={
          <HeaderIcon label="Search" onClick={openSearch}>
            <Search />
          </HeaderIcon>
        }
      />
      <div className="space-y-2 px-3 py-3">
        {projects.map((p) => (
          <Link key={p.id} to="/m/projects/$projectId" params={{ projectId: p.id }}>
            <ListCard className="active:bg-surface-2">
              <div className="flex items-center gap-3 px-3 py-3">
                <span className="grid size-9 shrink-0 place-items-center rounded-lg border border-border bg-surface-2">
                  <FolderClosed className="size-4 text-primary" />
                </span>
                <div className="min-w-0 flex-1">
                  <div className="truncate text-[14px] font-medium">{p.name}</div>
                  <div className="mt-1 flex flex-wrap items-center gap-2 text-[11.5px] text-muted-foreground">
                    {p.environments.map((e) => (
                      <span key={e.name} className="inline-flex items-center gap-1.5">
                        <EnvDot env={e.name} />
                        {e.name} · {e.count}
                      </span>
                    ))}
                  </div>
                </div>
                <ChevronRight className="size-4 shrink-0 text-subtle-foreground" />
              </div>
            </ListCard>
          </Link>
        ))}
      </div>
    </div>
  );
}
