import { createFileRoute, Link } from "@tanstack/react-router";
import { ChevronRight, FolderClosed, Plus } from "lucide-react";
import { toast } from "sonner";
import { projects } from "@/lib/envryn-data";
import { Button } from "@/components/envryn/ui";

export const Route = createFileRoute("/vault/projects/")({
  component: Projects,
});

function Projects() {
  return (
    <div className="min-h-full bg-background">
      <div className="content-wrap content-wrap--narrow">
        <header className="page-hero">
          <div>
            <p className="breadcrumb">
              Vault <span>/</span> Projects
            </p>
            <h1 className="mt-3 text-[22px] font-semibold tracking-[-0.035em]">Projects</h1>
            <p className="mt-1.5 text-[12.5px] text-muted-foreground">
              Keep credentials together by app and environment.
            </p>
          </div>
          <Button
            variant="primary"
            size="lg"
            onClick={() => toast("New project setup is ready to connect")}
          >
            <Plus />
            New project
          </Button>
        </header>

        <div className="mb-3 flex items-center justify-between border-y border-border/70 py-2.5 text-[11.5px] text-muted-foreground">
          <span>{projects.length} projects</span>
          <span>Choose a project to see its secrets</span>
        </div>

        <div className="project-list divide-y divide-border overflow-hidden rounded-lg border border-border bg-surface">
          {projects.map((project) => {
            const total = project.environments.reduce(
              (sum, environment) => sum + environment.count,
              0,
            );
            return (
              <Link
                key={project.id}
                to="/vault/projects/$projectId"
                params={{ projectId: project.id }}
                search={{ env: undefined }}
                className="project-row group flex items-center gap-4 px-4 py-4 transition-colors hover:bg-surface-2"
              >
                <span className="inline-flex size-8 shrink-0 items-center justify-center rounded-md border border-border bg-surface-2 text-muted-foreground">
                  <FolderClosed className="size-4" />
                </span>
                <span className="min-w-0 flex-1">
                  <span className="block text-[13px] font-medium text-foreground">
                    {project.name}
                  </span>
                  <span className="mt-2 flex flex-wrap gap-1.5">
                    {project.environments.map((environment) => (
                      <span key={environment.name} className="environment-chip">
                        <span
                          className={
                            environment.name === "Production"
                              ? "environment-dot environment-dot--production"
                              : environment.name === "Staging"
                                ? "environment-dot environment-dot--staging"
                                : "environment-dot"
                          }
                        />
                        {environment.name}{" "}
                        <span className="text-subtle-foreground">{environment.count}</span>
                      </span>
                    ))}
                  </span>
                </span>
                <span className="flex shrink-0 items-center gap-2 text-right">
                  <span>
                    <span className="block font-mono text-[13px] text-foreground">{total}</span>
                    <span className="block text-[10.5px] text-subtle-foreground">secrets</span>
                  </span>
                  <ChevronRight className="size-4 text-subtle-foreground transition-transform group-hover:translate-x-0.5 group-hover:text-foreground" />
                </span>
              </Link>
            );
          })}
        </div>
      </div>
    </div>
  );
}
