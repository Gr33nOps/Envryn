import * as React from "react";
import { toast } from "sonner";
import { projects, secretTypes, typeFields, type Secret } from "@/lib/envryn-data";
import { Button, Field, Input, Modal, Select } from "./ui";

export function SecretFormModal({
  open,
  onOpenChange,
  secret,
  preset,
}: {
  open: boolean;
  onOpenChange: (v: boolean) => void;
  secret?: Secret | null | undefined;
  preset?: Partial<Secret> | undefined;
}) {
  const editing = Boolean(secret);
  const [type, setType] = React.useState<string>("API Key");
  const [name, setName] = React.useState("");
  const [saving, setSaving] = React.useState(false);
  const [error, setError] = React.useState(false);

  React.useEffect(() => {
    if (!open) return;
    setType(secret?.type ?? preset?.type ?? "API Key");
    setName(secret?.name ?? "");
    setSaving(false);
    setError(false);
  }, [open, secret, preset]);

  const extra = typeFields[type] ?? [];

  return (
    <Modal
      open={open}
      onOpenChange={onOpenChange}
      width="sm:max-w-[460px]"
      title={editing ? "Edit secret" : "Add a secret"}
      description={
        editing
          ? "Update the details below."
          : "Save one credential and keep it easy to find later."
      }
      footer={
        <>
          <Button onClick={() => onOpenChange(false)}>Cancel</Button>
          <Button
            variant="primary"
            loading={saving}
            onClick={() => {
              if (!name.trim()) return setError(true);
              setSaving(true);
              setTimeout(() => {
                setSaving(false);
                onOpenChange(false);
                toast(editing ? "Secret updated" : "Secret saved");
              }, 550);
            }}
          >
            {editing ? "Save changes" : "Save secret"}
          </Button>
        </>
      }
    >
      <div className="space-y-4">
        <Field
          label="Name"
          hint="Use the name you will recognize in your project."
          error={error ? "Add a name so you can find this later." : undefined}
        >
          <Input
            mono
            autoFocus
            value={name}
            invalid={error}
            onChange={(event) => {
              setName(event.target.value);
              setError(false);
            }}
            placeholder="e.g. OPENAI_API_KEY"
          />
        </Field>

        <div className="grid grid-cols-1 gap-3 sm:grid-cols-3">
          <Field label="What kind is it?">
            <Select value={type} onChange={(event) => setType(event.target.value)}>
              {secretTypes.map((item) => (
                <option key={item}>{item}</option>
              ))}
            </Select>
          </Field>
          <Field label="Project">
            <Select defaultValue={secret?.project ?? preset?.project}>
              {projects.map((project) => (
                <option key={project.id}>{project.name}</option>
              ))}
              <option>Personal</option>
            </Select>
          </Field>
          <Field label="Environment" hint="Where will it be used?">
            <Select defaultValue={secret?.environment ?? preset?.environment}>
              <option>Development</option>
              <option>Staging</option>
              <option>Production</option>
              <option>—</option>
            </Select>
          </Field>
        </div>

        <Field
          label={type === "Note" ? "Note" : "Value"}
          hint={
            type === "Note"
              ? "Write anything you want to keep private."
              : "Paste the key, password, or token here."
          }
        >
          {type === "Note" ? (
            <textarea
              rows={4}
              className="w-full rounded-md border border-input bg-surface px-2 py-1.5 font-mono text-[12px] placeholder:text-subtle-foreground focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/25"
              placeholder="Write a private note..."
            />
          ) : (
            <Input mono type="password" placeholder="Paste secret value" />
          )}
        </Field>

        {extra.length > 0 && (
          <div className="rounded-md border border-border bg-surface-2/45 p-3">
            <p className="mb-2 text-[11px] font-medium text-muted-foreground">Optional details</p>
            <div className="grid grid-cols-2 gap-3">
              {extra.map((field) => (
                <Field key={field} label={field}>
                  <Input
                    mono={field !== "Provider"}
                    className="bg-surface"
                    placeholder="Optional"
                  />
                </Field>
              ))}
            </div>
          </div>
        )}

        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
          <Field label="Notes" hint="Anything helpful for you later.">
            <Input placeholder="Optional" />
          </Field>
          <Field label="Tags" hint="For example: work, backend.">
            <Input placeholder="Optional" />
          </Field>
        </div>
      </div>
    </Modal>
  );
}
