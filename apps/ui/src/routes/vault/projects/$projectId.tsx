import * as React from "react";
import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";
import { Plus, ChevronLeft, Pencil, Check, Trash2, Upload, X as XIcon } from "lucide-react";
import { toast } from "sonner";
import { type Environment, type Project, type Secret } from "@/lib/envryn-data";
import * as ipc from "@/lib/ipc";
import {
  useDeleteProject,
  useProjects,
  useRenameProject,
  useSecretList,
  useUpdateSecret,
} from "@/lib/use-vault";
import { SecretList } from "@/components/envryn/SecretList";
import { useVaultUI } from "@/components/envryn/vault-context";
import {
  Button,
  ConfirmDialog,
  EmptyState,
  IconButton,
  Input,
  PageHeader,
  SearchField,
  Select,
  Tabs,
} from "@/components/envryn/ui";

function NotFound({ projectId }: Readonly<{ projectId: string }>) {
  return (
    <div className="px-5 py-10">
      <EmptyState
        title="That project no longer exists"
        body={`No project with the identifier "${projectId}" is available in this vault.`}
        action={
          <Link to="/vault/projects">
            <Button variant="primary">Back to projects</Button>
          </Link>
        }
      />
    </div>
  );
}

export const Route = createFileRoute("/vault/projects/$projectId")({
  validateSearch: (s: Record<string, unknown>) => ({
    env: (s["env"] as string) || undefined,
  }),
  component: ProjectDetails,
});

/**
 * Renaming updates every filed secret and, for first-class projects, the
 * encrypted project metadata while preserving its stable route ID. Legacy
 * projects inferred from records continue to use a name-derived route slug.
 */
function ProjectTitle({ project, secrets }: Readonly<{ project: Project; secrets: Secret[] }>) {
  const updateSecret = useUpdateSecret();
  const renameProject = useRenameProject();
  const navigate = useNavigate();
  const [renaming, setRenaming] = React.useState(false);
  const [name, setName] = React.useState(project.name);
  const [saving, setSaving] = React.useState(false);

  React.useEffect(() => {
    if (!renaming) setName(project.name);
  }, [project.name, renaming]);

  function cancel() {
    setRenaming(false);
    setName(project.name);
  }

  async function save() {
    const trimmed = name.trim();
    if (!trimmed || trimmed === project.name) {
      cancel();
      return;
    }
    setSaving(true);
    try {
      const inProject = secrets.filter((s) => s.project === project.name);
      await Promise.all(
        inProject.map((s) => updateSecret.mutateAsync({ id: s.id, input: { project: trimmed } })),
      );
      const firstClass = /^[0-9a-f]{8}-[0-9a-f-]{27}$/i.test(project.id);
      if (firstClass) {
        await renameProject.mutateAsync({ id: project.id, name: trimmed });
      }
      setRenaming(false);
      toast(`Renamed to "${trimmed}"`);
      const newId = trimmed.toLowerCase().replace(/[^a-z0-9]+/g, "-");
      void navigate({
        to: "/vault/projects/$projectId",
        params: { projectId: firstClass ? project.id : newId },
        search: { env: undefined },
      });
    } catch (err) {
      toast(
        err instanceof ipc.IpcError
          ? err.message
          : "Could not rename every secret in this project.",
      );
    } finally {
      setSaving(false);
    }
  }

  if (renaming) {
    return (
      <span className="inline-flex items-center gap-1.5">
        <Input
          autoFocus
          value={name}
          disabled={saving}
          onChange={(event) => setName(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter") void save();
            if (event.key === "Escape") cancel();
          }}
          className="h-7 w-[220px] text-[13px]"
        />
        <IconButton label="Save name" onClick={() => void save()}>
          <Check />
        </IconButton>
        <IconButton label="Cancel rename" onClick={cancel}>
          <XIcon />
        </IconButton>
      </span>
    );
  }

  return (
    <span className="group inline-flex items-center gap-1.5">
      {project.name}
      <IconButton
        label="Rename project"
        // Hover-revealed on desktop; always visible on touch screens, which
        // have no hover, and when reached by keyboard.
        className="opacity-0 transition-opacity group-hover:opacity-100 focus-visible:opacity-100 [@media(hover:none)]:opacity-100"
        onClick={() => setRenaming(true)}
      >
        <Pencil />
      </IconButton>
    </span>
  );
}

function emptyTitle(query: string, environment: string, projectSecretCount: number): string {
  if (query) return `No results for "${query}"`;
  if (projectSecretCount === 0) return "No secrets yet";
  if (environment === "—") return "No secrets without an environment";
  return `No secrets in ${environment}`;
}

function deleteProjectBody(count: number): string {
  const what =
    count === 0
      ? "This removes the empty project."
      : `This removes the project and its ${count} secret${count === 1 ? "" : "s"}.`;
  return `${what} Paired devices remove them too when they next sync. This can't be undone.`;
}

function ProjectDetails() {
  const secrets = useSecretList();
  const projects = useProjects();
  const { projectId } = Route.useParams();
  const { env } = Route.useSearch();
  const { openAdd, openImport } = useVaultUI();
  const navigate = useNavigate();
  const deleteProject = useDeleteProject();
  const [confirmingDelete, setConfirmingDelete] = React.useState(false);
  const [q, setQ] = React.useState("");
  const [sort, setSort] = React.useState("name");

  // Projects include encrypted project metadata and legacy names inferred
  // from records, both of which are loaded through vault queries.
  const project = projects.find((p) => p.id === projectId);
  const [active, setActive] = React.useState<string | null>(env ?? null);

  if (!project) {
    // Still loading, or the project is genuinely gone.
    return secrets.length === 0 ? null : <NotFound projectId={projectId} />;
  }

  const currentEnvironment = active ?? project.environments[0]?.name ?? "—";
  const projectSecretCount = project.environments.reduce((sum, e) => sum + e.count, 0);

  const items = secrets
    .filter(
      (s) =>
        s.project === project.name &&
        s.environment === (currentEnvironment as Environment) &&
        s.name.toLowerCase().includes(q.trim().toLowerCase()),
    )
    .sort((a, b) =>
      sort === "name" ? a.name.localeCompare(b.name) : a.type.localeCompare(b.type),
    );

  return (
    <>
      <PageHeader
        title={<ProjectTitle project={project} secrets={secrets} />}
        back={
          <Link
            to="/vault/projects"
            className="mb-1 inline-flex items-center gap-1 text-[11.5px] text-muted-foreground hover:text-foreground"
          >
            <ChevronLeft className="size-3" />
            Projects
          </Link>
        }
        actions={
          <div className="project-actions flex items-center gap-2">
            <Button
              variant="secondary"
              aria-label="Delete project"
              title="Delete project"
              className="project-delete"
              onClick={() => setConfirmingDelete(true)}
            >
              <Trash2 />
            </Button>
            <Button
              variant="secondary"
              onClick={() =>
                openImport({
                  project: project.name,
                  environment: currentEnvironment as Environment,
                })
              }
            >
              <Upload />
              Import .env
            </Button>
            <Button
              variant="primary"
              onClick={() =>
                openAdd({ project: project.name, environment: currentEnvironment as Environment })
              }
            >
              <Plus />
              Add secret
            </Button>
          </div>
        }
      />

      <div className="space-y-3 px-5 pb-5">
        {project.environments.length > 0 && (
          <Tabs
            variant="segmented"
            items={project.environments.map((e) => ({
              value: e.name,
              label: e.name,
              count: e.count,
            }))}
            value={currentEnvironment}
            onChange={setActive}
          />
        )}

        <div className="flex items-center gap-2">
          <SearchField
            value={q}
            onChange={(e) => setQ(e.target.value)}
            placeholder={
              currentEnvironment === "—"
                ? "Search secrets..."
                : `Search ${currentEnvironment.toLowerCase()} secrets...`
            }
            className="max-w-[260px]"
          />
          <Select value={sort} onChange={(e) => setSort(e.target.value)} className="w-[130px]">
            <option value="name">Sort by name</option>
            <option value="type">Sort by type</option>
          </Select>
        </div>

        {items.length === 0 ? (
          <EmptyState
            title={emptyTitle(q, currentEnvironment, projectSecretCount)}
            body={
              q
                ? "Try another name, project, or tag."
                : projectSecretCount === 0
                  ? "Add a secret, or import a .env file."
                  : "Add a secret to this environment."
            }
            action={
              q ? undefined : (
                <div className="flex items-center justify-center gap-2">
                  <Button
                    variant="primary"
                    onClick={() =>
                      openAdd({
                        project: project.name,
                        environment: currentEnvironment as Environment,
                      })
                    }
                  >
                    <Plus />
                    Add secret
                  </Button>
                  <Button
                    variant="secondary"
                    onClick={() =>
                      openImport({
                        project: project.name,
                        environment: currentEnvironment as Environment,
                      })
                    }
                  >
                    <Upload />
                    Import .env
                  </Button>
                </div>
              )
            }
          />
        ) : (
          <SecretList items={items} columns={["project", "environment", "type", "updated"]} />
        )}
      </div>
      <ConfirmDialog
        open={confirmingDelete}
        onOpenChange={setConfirmingDelete}
        title={`Delete ${project.name}?`}
        body={deleteProjectBody(projectSecretCount)}
        confirmLabel="Delete project"
        onConfirm={() => {
          void (async () => {
            try {
              const removed = await deleteProject.mutateAsync(project.name);
              toast(
                `Deleted ${project.name}`,
                removed
                  ? { description: `${removed} secret${removed === 1 ? "" : "s"} removed.` }
                  : {},
              );
              void navigate({ to: "/vault/projects" });
            } catch (err) {
              toast(
                err instanceof ipc.IpcError ? err.message : "This project could not be deleted.",
              );
            }
          })();
        }}
      />
    </>
  );
}
