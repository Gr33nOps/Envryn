import * as React from "react";
import { Eye, EyeOff, Plus, Sparkles, Trash2 } from "lucide-react";
import { toast } from "sonner";
import { Button, Field, IconButton, Input, Modal, Select } from "@/components/envryn/ui";
import { secretTypes, typeFields, type Environment, type Secret } from "@/lib/envryn-data";
import { useCreateSecret, useProjects, useUpdateSecret } from "@/lib/use-vault";
import {
  buildSecretPayload,
  dateInputToMs,
  KIND_TO_TYPE,
  msToDateInput,
  payloadEditableFields,
  payloadPrimaryValue,
  tauriVaultRepository,
} from "@/lib/vault-repository";
import * as ipc from "@/lib/ipc";
import { IpcError } from "@/lib/ipc";

const ENVIRONMENTS: Environment[] = ["Development", "Staging", "Production", "—"];

function valueFieldHint(editing: boolean, isNote: boolean): string {
  if (editing) return "Leave blank to keep the value you already stored.";
  if (isNote) return "Write anything you want to keep private.";
  return "Paste the key, password, or token here.";
}

function saveValidationError({
  name,
  editing,
  type,
  value,
  existingPayload,
  customFields,
}: Readonly<{
  name: string;
  editing: boolean;
  type: string;
  value: string;
  existingPayload: ipc.SecretPayload | null;
  customFields: Array<{ label: string; value: string }>;
}>): string | null {
  if (!name.trim()) return "Add a name so you can find this later.";
  if (!editing && type !== "Custom" && !value) return "Paste the value you want to store.";
  if (editing && !existingPayload)
    return "Wait for the existing secret to finish loading before saving.";
  if (type === "Custom" && !customFields.some((field) => field.label.trim())) {
    return "Add at least one named custom field.";
  }
  return null;
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
          Suggest type
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
  const [expiresDate, setExpiresDate] = React.useState("");
  const [provider, setProvider] = React.useState("");
  const [fields, setFields] = React.useState<Record<string, string>>({});
  const [customFields, setCustomFields] = React.useState<{ label: string; value: string }[]>([]);
  const [existingPayload, setExistingPayload] = React.useState<ipc.SecretPayload | null>(null);
  const [loadingDetails, setLoadingDetails] = React.useState(false);
  const [saving, setSaving] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);
  const [suggesting, setSuggesting] = React.useState(false);
  const [suggestingName, setSuggestingName] = React.useState(false);
  const [showValue, setShowValue] = React.useState(false);

  React.useEffect(() => {
    if (!open) {
      setValue("");
      setExistingPayload(null);
      setFields({});
      setCustomFields([]);
      return;
    }
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
    setProvider(secret?.provider ?? "");
    setExpiresDate(
      secret?.expiresMs != null
        ? msToDateInput(secret.expiresMs)
        : preset?.expiresMs != null
          ? msToDateInput(preset.expiresMs)
          : "",
    );
    setFields({});
    setCustomFields(
      secret?.type === "Custom" || preset?.type === "Custom" ? [{ label: "", value: "" }] : [],
    );
    setExistingPayload(null);
    setLoadingDetails(Boolean(secret));
    setSaving(false);
    setError(null);
    setShowValue(false);

    if (!secret) return;
    let cancelled = false;
    void tauriVaultRepository
      .revealSecretRecord(secret.id)
      .then((record) => {
        if (cancelled) return;
        setType(KIND_TO_TYPE[record.payload.kind]);
        setName(record.name);
        setProject(record.project);
        setEnvironment(record.environment === "Unassigned" ? "—" : record.environment);
        setNotes(record.notes ?? "");
        setTags(record.tags.filter((tag) => tag.toLowerCase() !== "imported").join(", "));
        setProvider(record.provider ?? "");
        setExpiresDate(record.expires_ms != null ? msToDateInput(record.expires_ms) : "");
        setFields(payloadEditableFields(record.payload));
        setCustomFields(record.payload.kind === "Custom" ? record.payload.fields : []);
        setExistingPayload(record.payload);
        setValue(record.payload.kind === "Note" ? record.payload.body : "");
      })
      .catch((cause) => {
        if (!cancelled) {
          setError(cause instanceof Error ? cause.message : "Could not load this secret.");
        }
      })
      .finally(() => {
        if (!cancelled) setLoadingDetails(false);
      });
    return () => {
      cancelled = true;
    };
  }, [open, secret, preset]);

  const extra = typeFields[type] ?? [];

  /**
   * Rule-based: the value's known prefix or shape first, then the variable
   * name. Instant, offline, and nothing leaves the device. When the rules do
   * not recognise it, say "Unknown" instead of guessing -- the user picks.
   */
  async function suggestType() {
    if (!value.trim()) return;
    setSuggesting(true);
    try {
      const found = await ipc.classifyValue(value, name);
      if (!found) {
        toast("Type: Unknown", { description: "Choose the closest type from the list." });
        return;
      }
      const detected = KIND_TO_TYPE[found.kind];
      setType(detected);
      if (found.provider) setProvider(found.provider);
      toast(
        `Type: ${detected}`,
        found.provider ? { description: `Looks like a ${found.provider} credential.` } : {},
      );
    } catch (err) {
      toast(err instanceof IpcError ? err.message : "Could not check this value.");
    } finally {
      setSuggesting(false);
    }
  }

  /**
   * Rule-based name: a pasted `NAME=value` line names itself, and a
   * recognised key gets the name its service documents (`sk_live_...` is
   * `STRIPE_SECRET_KEY`). Anything else is "Unknown" and the name field is
   * left alone, so a secret is never saved as literally "Unknown".
   */
  async function suggestName() {
    if (!value.trim()) return;
    setSuggestingName(true);
    try {
      const suggested = await ipc.suggestSecretName(value);
      if (!suggested) {
        toast("Name: Unknown", { description: "Envryn doesn't recognise this key. Type a name." });
        return;
      }
      setName(suggested);
      setError(null);
      toast(`Name: ${suggested}`);
    } catch (err) {
      toast(err instanceof IpcError ? err.message : "Could not check this value.");
    } finally {
      setSuggestingName(false);
    }
  }

  async function save() {
    const validationError = saveValidationError({
      name,
      editing,
      type,
      value,
      existingPayload,
      customFields,
    });
    if (validationError) {
      setError(validationError);
      return;
    }

    const parsedTags = tags
      .split(",")
      .map((t) => t.trim())
      .filter(Boolean);

    // Empty field -> null, which clears any existing expiry on edit and stores
    // "does not expire" on create.
    const expiresMs = expiresDate ? dateInputToMs(expiresDate) : null;

    setSaving(true);
    try {
      const storedValue = value || (existingPayload ? payloadPrimaryValue(existingPayload) : "");
      const payload = buildSecretPayload(
        type as Secret["type"],
        storedValue,
        name,
        fields,
        type === "Custom"
          ? customFields
              .filter((field) => field.label.trim())
              .map((field) => ({ label: field.label.trim(), value: field.value }))
          : undefined,
      );
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
            provider,
            payload,
            expiresMs,
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
          provider,
          payload,
          expiresMs,
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
          <Button
            variant="primary"
            loading={saving || loadingDetails}
            disabled={loadingDetails}
            onClick={() => void save()}
          >
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
                  Suggest name
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
            <Select
              value={type}
              onChange={(event) => {
                setType(event.target.value);
                setFields({});
                setCustomFields(event.target.value === "Custom" ? [{ label: "", value: "" }] : []);
              }}
            >
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
          {type === "Custom" ? (
            <div className="space-y-2">
              {customFields.map((field, index) => (
                <div
                  key={index}
                  className="grid grid-cols-[minmax(0,0.8fr)_minmax(0,1.2fr)_32px] gap-2"
                >
                  <Input
                    value={field.label}
                    onChange={(event) =>
                      setCustomFields((current) =>
                        current.map((item, itemIndex) =>
                          itemIndex === index ? { ...item, label: event.target.value } : item,
                        ),
                      )
                    }
                    placeholder="Field name"
                    aria-label={`Custom field ${index + 1} name`}
                  />
                  <Input
                    mono
                    type={showValue ? "text" : "password"}
                    value={field.value}
                    onChange={(event) =>
                      setCustomFields((current) =>
                        current.map((item, itemIndex) =>
                          itemIndex === index ? { ...item, value: event.target.value } : item,
                        ),
                      )
                    }
                    placeholder="Value"
                    aria-label={`Custom field ${index + 1} value`}
                  />
                  <IconButton
                    label={`Remove custom field ${index + 1}`}
                    disabled={customFields.length === 1}
                    onClick={() =>
                      setCustomFields((current) =>
                        current.filter((_, itemIndex) => itemIndex !== index),
                      )
                    }
                  >
                    <Trash2 />
                  </IconButton>
                </div>
              ))}
              <div className="flex items-center justify-between">
                <Button
                  size="sm"
                  onClick={() =>
                    setCustomFields((current) => [...current, { label: "", value: "" }])
                  }
                >
                  <Plus />
                  Add field
                </Button>
                <IconButton
                  label={showValue ? "Hide values" : "Show values"}
                  onClick={() => setShowValue((visible) => !visible)}
                >
                  {showValue ? <EyeOff /> : <Eye />}
                </IconButton>
              </div>
            </div>
          ) : type === "Note" ? (
            <textarea
              aria-label="Note body"
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
          <div className="grid grid-cols-1 gap-3 rounded-md border border-border bg-surface-2/45 p-3 sm:grid-cols-2">
            {extra.map((label) => (
              <Field key={label} label={label}>
                <Input
                  mono={label !== "Provider"}
                  type={label === "Passphrase" ? "password" : label === "Port" ? "number" : "text"}
                  value={label === "Provider" ? provider : (fields[label] ?? "")}
                  onChange={(event) => {
                    if (label === "Provider") setProvider(event.target.value);
                    else setFields((current) => ({ ...current, [label]: event.target.value }));
                  }}
                  placeholder={label === "Port" ? "5432" : "Optional"}
                />
              </Field>
            ))}
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

        <Field
          label="Expires"
          hint="Optional. For keys that lapse -- an IGDB token (~60 days), a rotated password. We'll flag it as the date nears."
        >
          <div className="flex items-center gap-2">
            <Input
              type="date"
              value={expiresDate}
              onChange={(event) => setExpiresDate(event.target.value)}
              className="max-w-[190px]"
            />
            {expiresDate && (
              <button
                type="button"
                onClick={() => setExpiresDate("")}
                className="text-[11px] text-primary hover:text-foreground"
              >
                Clear
              </button>
            )}
          </div>
        </Field>
      </div>
    </Modal>
  );
}
