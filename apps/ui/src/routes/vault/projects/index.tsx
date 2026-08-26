import * as React from "react";
import { createFileRoute, Link } from "@tanstack/react-router";
import { ChevronRight, FolderClosed, Plus } from "lucide-react";
import { useProjects } from "@/lib/use-vault";
import { useVaultUI } from "@/components/envryn/vault-context";
import { Button, Field, Input, Modal } from "@/components/envryn/ui";

export const Route = createFileRoute("/vault/projects/")({
  component: Projects,
});

function environmentDotClass(name: string): string {
  if (name === "Production") return "environment-dot environment-dot--production";
  if (name === "Staging") return "environment-dot environment-dot--staging";
  return "environment-dot";
}

/**
 * There is no `project_create` command -- `useProjects` derives the list
 * entirely from secrets' own `project` field (see that hook's doc comment).
 * "New project" therefore means: name it here, then land on the normal Add
 * Secret form with that name already filled in, so the project exists the
 * same way every other project does -- by holding a secret.
 */
function NewProjectModal({
  open,
  onOpenChange,
  onNamed,
}: Readonly<{
  open: boolean;
  onOpenChange: (v: boolean) => void;
  onNamed: (name: string) => void;
}>) {
  const [name, setName] = React.useState("");

  React.useEffect(() => {
    if (open) setName("");
  }, [open]);

  function submit() {
    const trimmed = name.trim();
    if (!trimmed) return;
    onOpenChange(false);
    onNamed(trimmed);
  }

  return (
    <Modal
      open={open}
      onOpenChange={onOpenChange}
      title="New project"
      description="Give it a name, then add its first secret."
      footer={
        <>
          <Button onClick={() => onOpenChange(false)}>Cancel</Button>
          <Button variant="primary" disabled={!name.trim()} onClick={submit}>
            Continue
          </Button>
        </>
      }
    >
      <Field label="Project name" hint="You'll add its first secret next.">
        <Input
          autoFocus
          value={name}
          onChange={(event) => setName(event.target.value)}
          onKeyDown={(event) => event.key === "Enter" && submit()}
          placeholder="e.g. Rescripto"
        />
      </Field>
    </Modal>
  );
}

function Projects() {
  const projects = useProjects();
  const { openAdd } = useVaultUI();
  const [newProjectOpen, setNewProjectOpen] = React.useState(false);
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
          <Button variant="primary" size="lg" onClick={() => setNewProjectOpen(true)}>
            <Plus />
            New project
          </Button>
        </header>

        <NewProjectModal
          open={newProjectOpen}
          onOpenChange={setNewProjectOpen}
          onNamed={(name) => openAdd({ project: name })}
        />

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
                        <span className={environmentDotClass(environment.name)} />
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
