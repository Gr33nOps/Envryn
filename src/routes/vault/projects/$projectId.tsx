import * as React from "react";
import { createFileRoute, Link, notFound } from "@tanstack/react-router";
import { Plus, ChevronLeft } from "lucide-react";
import { projects, secrets, type Environment } from "@/lib/envryn-data";
import { SecretList } from "@/components/envryn/SecretList";
import { useVaultUI } from "@/components/envryn/vault-context";
import { Button, EmptyState, PageHeader, SearchField, Select, Tabs } from "@/components/envryn/ui";

export const Route = createFileRoute("/vault/projects/$projectId")({
  validateSearch: (s: Record<string, unknown>) => ({
    env: (s["env"] as string) || undefined,
  }),
  loader: ({ params }) => {
    const project = projects.find((p) => p.id === params.projectId);
    if (!project) throw notFound();
    return project;
  },
  component: ProjectDetails,
});

function ProjectDetails() {
  const project = Route.useLoaderData();
  const { env } = Route.useSearch();
  const { openAdd } = useVaultUI();
  const [active, setActive] = React.useState<string>(env ?? project.environments[0]!.name);
  const [q, setQ] = React.useState("");
  const [sort, setSort] = React.useState("name");

  const items = secrets
    .filter(
      (s) =>
        s.project === project.name &&
        s.environment === (active as Environment) &&
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
            onClick={() => openAdd({ project: project.name, environment: active as Environment })}
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
          value={active}
          onChange={setActive}
        />

        <div className="flex items-center gap-2">
          <SearchField
            value={q}
            onChange={(e) => setQ(e.target.value)}
            placeholder={`Search ${active.toLowerCase()} secrets...`}
            className="max-w-[260px]"
          />
          <Select value={sort} onChange={(e) => setSort(e.target.value)} className="w-[130px]">
            <option value="name">Sort by name</option>
            <option value="type">Sort by type</option>
          </Select>
        </div>

        {items.length === 0 ? (
          <EmptyState
            title={q ? `No results for "${q}"` : `No secrets in ${active}`}
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
