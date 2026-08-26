import * as React from "react";
import { Eye, EyeOff, Sparkles } from "lucide-react";
import { toast } from "sonner";
import { Button, Field, IconButton, Input, Modal, Select } from "@/components/envryn/ui";
import { secretTypes, typeFields, type Environment, type Secret } from "@/lib/envryn-data";
import { useCreateSecret, useProjects, useUpdateSecret } from "@/lib/use-vault";
import { KIND_TO_TYPE } from "@/lib/vault-repository";
import * as ipc from "@/lib/ipc";
import { IpcError } from "@/lib/ipc";

const ENVIRONMENTS: Environment[] = ["Development", "Staging", "Production", "—"];

function valueFieldHint(editing: boolean, isNote: boolean): string {
  if (editing) return "Leave blank to keep the value you already stored.";
  if (isNote) return "Write anything you want to keep private.";
  return "Paste the key, password, or token here.";
}

function ValueFieldLabel({
  showSuggestType,
  isNote,
  suggesting,
  onSuggestType,
}: Readonly<{
  showSuggestType: boolean;
  isNote: boolean;
  suggesting: boolean;
  onSuggestType: () => void;
}>) {
  if (showSuggestType) {
    return (
      <span className="flex items-center justify-between">
        <span>Value</span>
        <button
          type="button"
          onClick={onSuggestType}
          disabled={suggesting}
          className="inline-flex items-center gap-1 text-[10.5px] font-normal text-primary hover:text-foreground disabled:opacity-50"
        >
          <Sparkles className="size-3" />
          {suggesting ? "Checking..." : "Suggest type"}
        </button>
      </span>
    );
  }
  return <>{isNote ? "Note" : "Value"}</>;
}

export function SecretFormModal({
  open,
  onOpenChange,
  secret,
  preset,
}: Readonly<{
  open: boolean;
  onOpenChange: (v: boolean) => void;
  secret?: Secret | null | undefined;
  preset?: Partial<Secret> | undefined;
}>) {
  const projects = useProjects();
  const createSecret = useCreateSecret();
  const updateSecret = useUpdateSecret();
  const editing = Boolean(secret);

  const [type, setType] = React.useState<string>("API Key");
  const [name, setName] = React.useState("");
  const [project, setProject] = React.useState("");
  const [environment, setEnvironment] = React.useState<Environment>("Development");
  const [value, setValue] = React.useState("");
  const [notes, setNotes] = React.useState("");
  const [tags, setTags] = React.useState("");
  const [saving, setSaving] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);
  const [suggesting, setSuggesting] = React.useState(false);
  const [suggestingName, setSuggestingName] = React.useState(false);
  const [showValue, setShowValue] = React.useState(false);

  React.useEffect(() => {
    if (!open) return;
    setType(secret?.type ?? preset?.type ?? "API Key");
    setName(secret?.name ?? "");
    setProject(secret?.project ?? preset?.project ?? "");
    setEnvironment(secret?.environment ?? preset?.environment ?? "Development");
    // Never prefill the value when editing. A summary carries no secret
    // material, and fetching the plaintext just to populate a field the user
    // may not touch would put it on screen for no reason.
    setValue("");
    setNotes(secret?.notes ?? "");
    setTags((secret?.tags ?? []).join(", "));
    setSaving(false);
    setError(null);
    setShowValue(false);
  }, [open, secret, preset]);

  const extra = typeFields[type] ?? [];

  /**
   * Deterministic classification first -- known-prefix/shape matching that
   * works with no model installed (`docs/AI_DATA_ACCESS.md` section 3: "the
   * AI is the fallback for values the rules do not recognise"). Only if
   * that finds nothing does this fall back to the local model, and only if
   * the user has turned it on -- a failed or declined AI call here just
   * means no suggestion, never a blocked save.
   */
  async function suggestType() {
    if (!value.trim()) return;
    setSuggesting(true);
    try {
      const deterministic = await ipc.classifyDeterministic(value);
      if (deterministic) {
        setType(KIND_TO_TYPE[deterministic.kind]);
        toast(
          deterministic.provider
            ? `Looks like a ${deterministic.provider} credential`
            : "Type detected",
        );
        return;
      }
      const status = await ipc.aiStatus().catch(() => null);
      if (!status?.enabled_in_settings || !status.engine_running) {
        toast("Couldn't recognize this value automatically. Enable local AI in Settings for more.");
        return;
      }
      const result = await ipc.aiClassifyPastedValue(value);
      setType(KIND_TO_TYPE[result.kind]);
      toast(
        result.provider
          ? `Local AI: looks like a ${result.provider} credential`
          : "Local AI suggested a type",
      );
    } catch (err) {
      toast(err instanceof IpcError ? err.message : "Could not suggest a type for this value.");
    } finally {
      setSuggesting(false);
    }
  }

  /**
   * L2, same data-access level as `suggestType`: the pasted value plus
   * whatever provider deterministic classification already found (never a
   * second round of AI-only detection just for this). Unlike type
   * detection, there is no non-AI fallback for naming -- gated entirely on
   * local AI being enabled and running, same as `docs/AI_DATA_ACCESS.md`'s
   * Tier 1 "naming" row describes.
   */
  async function suggestName() {
    if (!value.trim()) return;
    setSuggestingName(true);
    try {
      const status = await ipc.aiStatus().catch(() => null);
      if (!status?.enabled_in_settings || !status.engine_running) {
        toast("Enable local AI in Settings to get name suggestions.");
        return;
      }
      const deterministic = await ipc.classifyDeterministic(value).catch(() => null);
      const result = await ipc.aiSuggestName(value, deterministic?.provider ?? null);
      setName(result.name);
      toast("Suggested a name based on this value.");
    } catch (err) {
      toast(err instanceof IpcError ? err.message : "Could not suggest a name for this value.");
    } finally {
      setSuggestingName(false);
    }
  }

  async function save() {
    if (!name.trim()) {
      setError("Add a name so you can find this later.");
      return;
    }
    if (!editing && !value) {
      setError("Paste the value you want to store.");
      return;
    }

    const parsedTags = tags
      .split(",")
      .map((t) => t.trim())
      .filter(Boolean);

    setSaving(true);
    try {
      if (editing && secret) {
        await updateSecret.mutateAsync({
          id: secret.id,
          input: {
            name: name.trim(),
            project: project.trim(),
            environment,
            type: type as Secret["type"],
            notes,
            tags: parsedTags,
            // Only send a value when the user actually typed one, so leaving
            // the field blank means "keep the existing secret" rather than
            // silently overwriting it with an empty string.
            ...(value ? { value } : {}),
          },
        });
      } else {
        await createSecret.mutateAsync({
          name: name.trim(),
          project: project.trim(),
          environment,
          type: type as Secret["type"],
          value,
          notes,
          tags: parsedTags,
        });
      }
      // Drop the plaintext from component state as soon as it is stored.
      setValue("");
      onOpenChange(false);
      toast(editing ? "Secret updated" : "Secret saved");
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
          <Button variant="primary" loading={saving} onClick={() => void save()}>
            {editing ? "Save changes" : "Save secret"}
          </Button>
        </>
      }
    >
      <div className="space-y-4">
        <Field
          label={
            !editing && type !== "Note" && value.trim() ? (
              <span className="flex items-center justify-between">
                <span>Name</span>
                <button
                  type="button"
                  onClick={() => void suggestName()}
                  disabled={suggestingName}
                  className="inline-flex items-center gap-1 text-[10.5px] font-normal text-primary hover:text-foreground disabled:opacity-50"
                >
                  <Sparkles className="size-3" />
                  {suggestingName ? "Thinking..." : "Suggest name"}
                </button>
              </span>
            ) : (
              "Name"
            )
          }
          hint="Use the name you will recognize in your project."
          error={error ?? undefined}
        >
          <Input
            mono
            autoFocus
            value={name}
            invalid={Boolean(error)}
            onChange={(event) => {
              setName(event.target.value);
              setError(null);
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
          <Field label="Project" hint="Type a new name to start a project.">
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
          <Field label="Environment" hint="Where will it be used?">
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

        <Field
          label={
            <ValueFieldLabel
              showSuggestType={!editing && type !== "Note" && Boolean(value.trim())}
              isNote={type === "Note"}
              suggesting={suggesting}
              onSuggestType={() => void suggestType()}
            />
          }
          hint={valueFieldHint(editing, type === "Note")}
        >
          {type === "Note" ? (
            <textarea
              rows={4}
              value={value}
              onChange={(event) => setValue(event.target.value)}
              className="w-full rounded-md border border-input bg-surface px-2 py-1.5 font-mono text-[12px] placeholder:text-subtle-foreground focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/25"
              placeholder="Write a private note..."
            />
          ) : (
            <div className="relative">
              <Input
                mono
                type={showValue ? "text" : "password"}
                className="pr-9"
                value={value}
                onChange={(event) => setValue(event.target.value)}
                placeholder={editing ? "Unchanged" : "Paste secret value"}
              />
              <IconButton
                label={showValue ? "Hide value" : "Show value"}
                className="absolute right-1 top-1/2 -translate-y-1/2"
                onClick={() => setShowValue((visible) => !visible)}
              >
                {showValue ? <EyeOff /> : <Eye />}
              </IconButton>
            </div>
          )}
        </Field>

        {extra.length > 0 && (
          <div className="rounded-md border border-border bg-surface-2/45 p-3">
            <p className="mb-2 text-[11px] font-medium text-muted-foreground">Optional details</p>
            <p className="mb-2 text-[10.5px] text-subtle-foreground">
              Separate fields for {extra.join(", ").toLowerCase()} arrive with the structured form.
              For now the whole value is stored as one entry.
            </p>
          </div>
        )}

        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
          <Field label="Notes" hint="Anything helpful for you later.">
            <Input
              value={notes}
              onChange={(event) => setNotes(event.target.value)}
              placeholder="Optional"
            />
          </Field>
          <Field label="Tags" hint="Comma separated. For example: work, backend.">
            <Input
              value={tags}
              onChange={(event) => setTags(event.target.value)}
              placeholder="Optional"
            />
          </Field>
        </div>
      </div>
    </Modal>
  );
}
