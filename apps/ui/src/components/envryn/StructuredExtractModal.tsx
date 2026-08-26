import * as React from "react";
import { Plus, ShieldAlert, Trash2 } from "lucide-react";
import { toast } from "sonner";
import { Button, Field, Input, Modal, Select } from "@/components/envryn/ui";
import { type Environment } from "@/lib/envryn-data";
import { useCreateSecret, useProjects } from "@/lib/use-vault";
import * as ipc from "@/lib/ipc";
import { IpcError } from "@/lib/ipc";

const ENVIRONMENTS: Environment[] = ["Development", "Staging", "Production", "—"];

interface FieldRow {
  id: number;
  label: string;
  value: string;
}

let nextFieldRowId = 0;
function newFieldRow(label = "", value = ""): FieldRow {
  nextFieldRowId += 1;
  return { id: nextFieldRowId, label, value };
}

/**
 * Level 3 (`docs/AI_DATA_ACCESS.md`): the whole pasted block is genuinely the
 * input, so unlike classification/naming there is no deterministic path to
 * try first, and no way to avoid sending the plaintext block to the local
 * model. Confirmation is required every time (never remembered), and the
 * notice below names exactly what is about to happen before it does --
 * the "data-access indicator" the spec calls for at this level.
 */
export function StructuredExtractModal({
  open,
  onOpenChange,
}: Readonly<{
  open: boolean;
  onOpenChange: (v: boolean) => void;
}>) {
  const projects = useProjects();
  const createSecret = useCreateSecret();

  const [stage, setStage] = React.useState<"paste" | "review">("paste");
  const [block, setBlock] = React.useState("");
  const [name, setName] = React.useState("");
  const [project, setProject] = React.useState("");
  const [environment, setEnvironment] = React.useState<Environment>("Development");
  const [fields, setFields] = React.useState<FieldRow[]>([]);
  const [extracting, setExtracting] = React.useState(false);
  const [saving, setSaving] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);

  React.useEffect(() => {
    if (!open) return;
    setStage("paste");
    setBlock("");
    setName("");
    setProject("");
    setEnvironment("Development");
    setFields([]);
    setError(null);
  }, [open]);

  async function extract() {
    if (!block.trim()) {
      setError("Paste the text you want fields extracted from.");
      return;
    }
    setError(null);
    setExtracting(true);
    try {
      const status = await ipc.aiStatus();
      if (!status.enabled_in_settings || !status.engine_running) {
        setError("Enable local AI in Settings to extract fields from text.");
        return;
      }
      const result = await ipc.aiExtractStructuredFields(block);
      if (result.fields.length === 0) {
        setError("No labeled fields were found in that text.");
        return;
      }
      setFields(result.fields.map((f) => newFieldRow(f.label, f.value)));
      setStage("review");
    } catch (err) {
      setError(err instanceof IpcError ? err.message : "Could not extract fields from that text.");
    } finally {
      setExtracting(false);
    }
  }

  function updateField(id: number, patch: Partial<FieldRow>) {
    setFields((prev) => prev.map((f) => (f.id === id ? { ...f, ...patch } : f)));
  }

  function removeField(id: number) {
    setFields((prev) => prev.filter((f) => f.id !== id));
  }

  function addField() {
    setFields((prev) => [...prev, newFieldRow()]);
  }

  async function save() {
    if (!name.trim()) {
      setError("Add a name so you can find this later.");
      return;
    }
    if (!project.trim()) {
      setError("Name a project for this secret.");
      return;
    }
    const cleaned = fields.filter((f) => f.label.trim() && f.value.trim());
    if (cleaned.length === 0) {
      setError("At least one field needs both a label and a value.");
      return;
    }

    setSaving(true);
    try {
      await createSecret.mutateAsync({
        name: name.trim(),
        project: project.trim(),
        environment,
        type: "Custom",
        value: "",
        notes: "",
        tags: [],
        customFields: cleaned,
      });
      toast(
        "Secret saved with " + cleaned.length + " field" + (cleaned.length === 1 ? "" : "s") + ".",
      );
      onOpenChange(false);
    } catch (err) {
      setError(
        err instanceof IpcError ? err.message : "That could not be saved. Nothing was changed.",
      );
    } finally {
      setSaving(false);
    }
  }

  return (
    <Modal
      open={open}
      onOpenChange={onOpenChange}
      width={stage === "review" ? "sm:max-w-[560px]" : "sm:max-w-[460px]"}
      title="Extract fields from text"
      description={
        stage === "paste"
          ? "Paste a config block, connection string, or similar and let local AI pull out labeled fields."
          : "Review the extracted fields before saving."
      }
      footer={
        stage === "paste" ? (
          <>
            <Button onClick={() => onOpenChange(false)}>Cancel</Button>
            <Button variant="primary" loading={extracting} onClick={() => void extract()}>
              {extracting ? "Extracting..." : "Extract fields"}
            </Button>
          </>
        ) : (
          <>
            <Button onClick={() => setStage("paste")}>Back</Button>
            <Button variant="primary" loading={saving} onClick={() => void save()}>
              Save secret
            </Button>
          </>
        )
      }
    >
      {stage === "paste" ? (
        <div className="space-y-4">
          <div className="flex items-start gap-2 rounded-md border border-border bg-surface-2/60 px-2.5 py-2 text-[11px] leading-relaxed text-muted-foreground">
            <ShieldAlert className="mt-0.5 size-3.5 shrink-0 text-subtle-foreground" />
            <span>
              This text is sent to your local AI model to find labeled fields. It never leaves this
              device, and nothing is saved until you review and confirm below.
            </span>
          </div>
          <Field
            label="Text to extract from"
            hint="Anything with labeled values -- a config dump, a connection string, an email with credentials."
            error={error ?? undefined}
          >
            <textarea
              autoFocus
              rows={8}
              value={block}
              onChange={(event) => {
                setBlock(event.target.value);
                setError(null);
              }}
              className="w-full rounded-md border border-input bg-surface px-2 py-1.5 font-mono text-[12px] placeholder:text-subtle-foreground focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/25"
              placeholder={"Host: db.example.com\nPort: 5432\nUsername: admin"}
            />
          </Field>
        </div>
      ) : (
        <div className="space-y-4">
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-3">
            <Field label="Name" error={error ?? undefined} className="sm:col-span-1">
              <Input
                value={name}
                onChange={(event) => {
                  setName(event.target.value);
                  setError(null);
                }}
                placeholder="e.g. Staging DB"
              />
            </Field>
            <Field label="Project">
              <Input
                value={project}
                list="envryn-projects"
                onChange={(event) => setProject(event.target.value)}
                placeholder="e.g. Rescripto"
              />
              <datalist id="envryn-projects">
                {projects.map((p) => (
                  <option key={p.id} value={p.name} />
                ))}
              </datalist>
            </Field>
            <Field label="Environment">
              <Select
                value={environment}
                onChange={(event) => setEnvironment(event.target.value as Environment)}
              >
                {ENVIRONMENTS.map((env) => (
                  <option key={env}>{env}</option>
                ))}
              </Select>
            </Field>
          </div>

          <div className="space-y-2">
            <p className="text-[11px] font-medium text-muted-foreground">Fields</p>
            {fields.map((field) => (
              <div key={field.id} className="flex items-center gap-1.5">
                <Input
                  value={field.label}
                  onChange={(event) => updateField(field.id, { label: event.target.value })}
                  placeholder="Label"
                  className="w-[130px] shrink-0"
                />
                <Input
                  mono
                  value={field.value}
                  onChange={(event) => updateField(field.id, { value: event.target.value })}
                  placeholder="Value"
                  className="min-w-0 flex-1"
                />
                <button
                  type="button"
                  onClick={() => removeField(field.id)}
                  aria-label="Remove field"
                  className="inline-flex size-6 shrink-0 items-center justify-center rounded-md text-subtle-foreground hover:bg-surface-3 hover:text-destructive"
                >
                  <Trash2 className="size-3.5" />
                </button>
              </div>
            ))}
            <Button variant="ghost" size="sm" onClick={addField}>
              <Plus className="size-3.5" />
              Add field
            </Button>
          </div>
        </div>
      )}
    </Modal>
  );
}
