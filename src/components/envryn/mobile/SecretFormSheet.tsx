import * as React from "react";
import { toast } from "sonner";
import { cn } from "@/lib/utils";
import {
  projects,
  secretTypes,
  typeFields,
  type Secret,
  type SecretType,
} from "@/lib/envryn-data";
import { Sheet, TouchButton, MobileInput, MobileField } from "./Sheet";

const envs = ["Development", "Staging", "Production", "—"];

export function SecretFormSheet({
  open,
  onOpenChange,
  secret,
  preset,
}: {
  open: boolean;
  onOpenChange: (v: boolean) => void;
  secret?: Secret | null;
  preset?: Partial<Secret> | undefined;
}) {
  const editing = !!secret;
  const [type, setType] = React.useState<SecretType>("API Key");
  const [name, setName] = React.useState("");
  const [project, setProject] = React.useState("Rescripto");
  const [env, setEnv] = React.useState("Development");

  React.useEffect(() => {
    if (!open) return;
    setType((secret?.type ?? preset?.type ?? "API Key") as SecretType);
    setName(secret?.name ?? "");
    setProject(secret?.project ?? preset?.project ?? "Rescripto");
    setEnv((secret?.environment ?? preset?.environment ?? "Development") as string);
  }, [open, secret, preset]);

  return (
    <Sheet
      open={open}
      onOpenChange={onOpenChange}
      full
      title={editing ? "Edit secret" : "New secret"}
      description={
        editing ? "Changes are encrypted on this device." : "Pick a template to start."
      }
      footer={
        <>
          <TouchButton onClick={() => onOpenChange(false)}>Cancel</TouchButton>
          <TouchButton
            variant="primary"
            onClick={() => {
              onOpenChange(false);
              toast(editing ? "Secret updated" : "Secret saved");
            }}
          >
            {editing ? "Save changes" : "Save secret"}
          </TouchButton>
        </>
      }
    >
      <div className="space-y-4 pt-1">
        <MobileField label="Template">
          <div className="flex flex-wrap gap-1.5">
            {secretTypes.map((t) => (
              <button
                key={t}
                onClick={() => setType(t)}
                className={cn(
                  "h-8 rounded-lg border px-3 text-[12.5px] transition-colors",
                  t === type
                    ? "border-primary bg-primary-muted text-foreground"
                    : "border-border bg-surface-2 text-muted-foreground",
                )}
              >
                {t}
              </button>
            ))}
          </div>
        </MobileField>

        <MobileField label="Name">
          <MobileInput
            mono
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="STRIPE_SECRET_KEY"
          />
        </MobileField>

        <MobileField label="Value" hint="Stored encrypted, never leaves your devices.">
          <textarea
            rows={type === "Note" ? 6 : 3}
            placeholder={type === "Note" ? "Write your secure note…" : "sk_live_…"}
            className="w-full resize-none rounded-xl border border-input bg-background px-3 py-2.5 font-mono text-[12.5px] text-foreground placeholder:text-subtle-foreground focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/25"
          />
        </MobileField>

        <div className="grid grid-cols-2 gap-3">
          <MobileField label="Project">
            <select
              value={project}
              onChange={(e) => setProject(e.target.value)}
              className="h-11 w-full appearance-none rounded-xl border border-input bg-background px-3 text-[13px] focus:border-primary focus:outline-none"
            >
              {projects.map((p) => (
                <option key={p.id}>{p.name}</option>
              ))}
              <option>Personal</option>
            </select>
          </MobileField>
          <MobileField label="Environment">
            <select
              value={env}
              onChange={(e) => setEnv(e.target.value)}
              className="h-11 w-full appearance-none rounded-xl border border-input bg-background px-3 text-[13px] focus:border-primary focus:outline-none"
            >
              {envs.map((e2) => (
                <option key={e2}>{e2}</option>
              ))}
            </select>
          </MobileField>
        </div>

        {(typeFields[type] ?? []).map((f) => (
          <MobileField key={f} label={f}>
            <MobileInput placeholder={f} />
          </MobileField>
        ))}

        <MobileField label="Tags">
          <MobileInput placeholder="api, billing" />
        </MobileField>
      </div>
    </Sheet>
  );
}
