import * as React from "react";
import { createFileRoute, Link } from "@tanstack/react-router";
import { Plus, ChevronLeft } from "lucide-react";
import { type Environment } from "@/lib/envryn-data";
import { useProjects, useSecretList } from "@/lib/use-vault";
import { SecretList } from "@/components/envryn/SecretList";
import { useVaultUI } from "@/components/envryn/vault-context";
import { Button, EmptyState, PageHeader, SearchField, Select, Tabs } from "@/components/envryn/ui";

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
        title={project.name}
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
