import * as React from "react";
import { toast } from "sonner";
import { secretTypes, typeFields, projects, type Secret } from "@/lib/envryn-data";
import { Button, Field, Input, Modal, Select } from "./ui";

export function SecretFormModal({
  open,
  onOpenChange,
  secret,
  preset,
}: {
  open: boolean;
  onOpenChange: (v: boolean) => void;
  secret?: Secret | null;
  preset?: Partial<Secret>;
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
      title={editing ? "Edit Secret" : "Add Secret"}
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
            {editing ? "Save Changes" : "Save Secret"}
          </Button>
        </>
      }
    >
      <div className="space-y-3">
        <Field
          label="Name"
          error={error ? "Name is required." : undefined}
        >
          <Input
            mono
            autoFocus
            value={name}
            invalid={error}
            onChange={(e) => {
              setName(e.target.value);
              setError(false);
            }}
            placeholder="GROQ_API_KEY"
          />
        </Field>

        <div className="grid grid-cols-3 gap-3">
          <Field label="Type">
            <Select value={type} onChange={(e) => setType(e.target.value)}>
              {secretTypes.map((t) => (
                <option key={t}>{t}</option>
              ))}
            </Select>
          </Field>
          <Field label="Project">
            <Select defaultValue={secret?.project ?? preset?.project}>
              {projects.map((p) => (
                <option key={p.id}>{p.name}</option>
              ))}
              <option>Personal</option>
            </Select>
          </Field>
          <Field label="Environment">
            <Select defaultValue={secret?.environment ?? preset?.environment}>
              <option>Development</option>
              <option>Staging</option>
              <option>Production</option>
              <option>—</option>
            </Select>
          </Field>
        </div>

        <Field label={type === "Note" ? "Content" : "Secret Value"}>
          {type === "Note" ? (
            <textarea
              rows={4}
              className="w-full rounded-md border border-input bg-surface px-2 py-1.5 font-mono text-[12px] placeholder:text-subtle-foreground focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/25"
              placeholder="Content"
            />
          ) : (
            <Input mono type="password" defaultValue="••••••••••••••••••" />
          )}
        </Field>

        {extra.length > 0 && (
          <div className="grid grid-cols-2 gap-3 rounded-md border border-border bg-surface-2/50 p-2.5">
            {extra.map((f) => (
              <Field key={f} label={f}>
                <Input
                  mono={f !== "Provider"}
                  className="bg-surface"
                  placeholder="Optional"
                />
              </Field>
            ))}
          </div>
        )}

        <div className="grid grid-cols-2 gap-3">
          <Field label="Notes">
            <Input placeholder="Optional" />
          </Field>
          <Field label="Tags">
            <Input placeholder="Optional" />
          </Field>
        </div>
      </div>
    </Modal>
  );
}
