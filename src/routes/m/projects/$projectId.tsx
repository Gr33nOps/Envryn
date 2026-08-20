import * as React from "react";
import { createFileRoute, useParams } from "@tanstack/react-router";
import { Plus } from "lucide-react";
import { cn } from "@/lib/utils";
import { projects, secrets } from "@/lib/envryn-data";
import { HeaderIcon, MobileHeader } from "@/components/envryn/mobile/Shell";
import { SecretRow, EnvDot } from "@/components/envryn/mobile/SecretRow";
import { ListCard } from "@/components/envryn/mobile/Sheet";
import { useMobileUI } from "@/components/envryn/mobile/mobile-context";

export const Route = createFileRoute("/m/projects/$projectId")({
  head: () => ({
    meta: [
      { title: "Project secrets — Envryn Mobile" },
      {
        name: "description",
        content:
          "View a project's secrets per environment: development, staging and production keys and credentials.",
      },
      { property: "og:title", content: "Project secrets — Envryn Mobile" },
      {
        property: "og:description",
        content: "Environment-scoped secrets for a single project.",
      },
      { property: "og:type", content: "website" },
      { name: "twitter:card", content: "summary_large_image" },
    ],
  }),
  component: MobileProjectDetail,
});

function MobileProjectDetail() {
  const { projectId } = useParams({ from: "/m/projects/$projectId" });
  const project = projects.find((p) => p.id === projectId);
  const { openAdd } = useMobileUI();
  const envs = project?.environments.map((e) => e.name) ?? [];
  const [env, setEnv] = React.useState<string>(envs[0] ?? "Development");

  const items = secrets.filter(
    (s) => s.project === project?.name && s.environment === env,
  );

  return (
    <div className="pb-6">
      <MobileHeader
        title={project?.name ?? "Project"}
        subtitle={`${envs.length} environments`}
        back="/m/projects"
        right={
          <HeaderIcon
            label="Add secret"
            onClick={() =>
              openAdd({
                project: project?.name,
                environment: env as never,
              })
            }
          >
            <Plus />
          </HeaderIcon>
        }
      />

      <div className="flex gap-1.5 overflow-x-auto px-3 py-2.5 [scrollbar-width:none]">
        {envs.map((e) => (
          <button
            key={e}
            onClick={() => setEnv(e)}
            className={cn(
              "inline-flex h-8 shrink-0 items-center gap-1.5 rounded-lg border px-3 text-[12.5px]",
              e === env
                ? "border-primary bg-primary-muted text-foreground"
                : "border-border bg-surface text-muted-foreground",
            )}
          >
            <EnvDot env={e} />
            {e}
          </button>
        ))}
      </div>

      {env === "Production" && (
        <div className="mx-3 mb-2 rounded-xl border border-warning/35 bg-warning/8 px-3 py-2 text-[12px] text-warning">
          Production values require identity confirmation on every reveal.
        </div>
      )}

      <div className="px-3">
        {items.length === 0 ? (
          <div className="rounded-xl border border-dashed border-border px-4 py-14 text-center text-[13px] text-muted-foreground">
            No secrets in {env} yet.
          </div>
        ) : (
          <ListCard>
            {items.map((s) => (
              <SecretRow key={s.id} secret={s} />
            ))}
          </ListCard>
        )}
      </div>
    </div>
  );
}
