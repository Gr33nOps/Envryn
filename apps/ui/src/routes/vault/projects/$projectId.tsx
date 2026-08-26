import * as React from "react";
import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";
import { Plus, ChevronLeft, Pencil, Check, X as XIcon } from "lucide-react";
import { toast } from "sonner";
import { type Environment, type Project, type Secret } from "@/lib/envryn-data";
import * as ipc from "@/lib/ipc";
import { useProjects, useSecretList, useUpdateSecret } from "@/lib/use-vault";
import { SecretList } from "@/components/envryn/SecretList";
import { useVaultUI } from "@/components/envryn/vault-context";
import {
  Button,
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
        body={`Nothing is filed under "${projectId}" any more. A project exists for as long as it holds at least one secret.`}
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
 * Renaming a project means bulk-updating the `project` field on every
 * secret filed under it -- there is no project row to rename, per
 * `useProjects`'s own doc comment. The URL's `$projectId` is a slug of the
 * name, so a successful rename also navigates to the new slug.
 */
function ProjectTitle({ project, secrets }: Readonly<{ project: Project; secrets: Secret[] }>) {
  const updateSecret = useUpdateSecret();
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
      setRenaming(false);
      toast(`Renamed to "${trimmed}"`);
      const newId = trimmed.toLowerCase().replace(/[^a-z0-9]+/g, "-");
      void navigate({
        to: "/vault/projects/$projectId",
        params: { projectId: newId },
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
        className="opacity-0 transition-opacity group-hover:opacity-100"
        onClick={() => setRenaming(true)}
      >
        <Pencil />
      </IconButton>
    </span>
  );
}

function ProjectDetails() {
  const secrets = useSecretList();
  const projects = useProjects();
  const { projectId } = Route.useParams();
  const { env } = Route.useSearch();
  const { openAdd } = useVaultUI();
  const [q, setQ] = React.useState("");
  const [sort, setSort] = React.useState("name");

  // Projects are derived from the records filed under them, so a project
  // cannot be resolved in a route loader -- it only exists once the record
  // list has loaded. Resolving here also means deleting the last secret in a
  // project correctly makes the project disappear.
  const project = projects.find((p) => p.id === projectId);
  const [active, setActive] = React.useState<string | null>(env ?? null);

  if (!project) {
    // Still loading, or the project is genuinely gone.
    return secrets.length === 0 ? null : <NotFound projectId={projectId} />;
  }

  const currentEnvironment = active ?? project.environments[0]?.name ?? "—";

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
          <Button
            variant="primary"
            onClick={() =>
              openAdd({ project: project.name, environment: currentEnvironment as Environment })
            }
          >
            <Plus />
            Add secret
          </Button>
        }
      />

      <div className="space-y-3 px-5 pb-5">
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

        <div className="flex items-center gap-2">
          <SearchField
            value={q}
            onChange={(e) => setQ(e.target.value)}
            placeholder={`Search ${currentEnvironment.toLowerCase()} secrets...`}
            className="max-w-[260px]"
          />
          <Select value={sort} onChange={(e) => setSort(e.target.value)} className="w-[130px]">
            <option value="name">Sort by name</option>
            <option value="type">Sort by type</option>
          </Select>
        </div>

        {items.length === 0 ? (
          <EmptyState
            title={q ? `No results for "${q}"` : `No secrets in ${currentEnvironment}`}
            body={q ? "Try another name, project, or tag." : "Add a secret to this environment."}
            action={
              q ? undefined : (
                <Button variant="primary" onClick={() => openAdd()}>
                  <Plus />
                  Add secret
                </Button>
              )
            }
          />
        ) : (
          <SecretList items={items} columns={["project", "environment", "type", "updated"]} />
        )}
      </div>
    </>
  );
}
