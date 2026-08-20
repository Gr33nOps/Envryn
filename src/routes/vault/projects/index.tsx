import { createFileRoute, Link } from "@tanstack/react-router";
import { Plus, ChevronRight } from "lucide-react";
import { projects } from "@/lib/envryn-data";
import { Button, PageHeader, Panel } from "@/components/envryn/ui";
import { cn } from "@/lib/utils";

export const Route = createFileRoute("/vault/projects/")({
  component: Projects,
});

function Projects() {
  return (
    <>
      <PageHeader
        title="Projects"
        actions={
          <Button variant="primary">
            <Plus />
            New Project
          </Button>
        }
      />
      <div className="space-y-2.5 px-5 pb-5">
        {projects.map((p) => (
          <Panel key={p.id}>
            <Link
              to="/vault/projects/$projectId"
              params={{ projectId: p.id }}
              className="group flex h-[34px] items-center justify-between gap-3 border-b border-border px-3 transition-colors hover:bg-surface-2/60"
            >
              <span className="text-[12.5px] font-medium">{p.name}</span>
              <span className="flex items-center gap-2 text-[11.5px] text-subtle-foreground">
                {p.environments.reduce((n, e) => n + e.count, 0)} secrets
                <ChevronRight className="size-3.5 opacity-0 transition-opacity group-hover:opacity-100" />
              </span>
            </Link>
            <ul>
              {p.environments.map((e) => (
                <li key={e.name}>
                  <Link
                    to="/vault/projects/$projectId"
                    params={{ projectId: p.id }}
                    search={{ env: e.name }}
                    className="flex h-[30px] items-center gap-2 border-b border-border/50 px-3 text-[12px] text-muted-foreground transition-colors last:border-0 hover:bg-surface-2/50 hover:text-foreground"
                  >
                    <span
                      className={cn(
                        "size-1.5 rounded-full",
                        e.name === "Production"
                          ? "bg-warning"
                          : e.name === "Staging"
                            ? "bg-primary"
                            : "bg-subtle-foreground",
                      )}
                    />
                    {e.name}
                    <span className="text-subtle-foreground">· {e.count} secrets</span>
                  </Link>
                </li>
              ))}
            </ul>
          </Panel>
        ))}
      </div>
    </>
  );
}
