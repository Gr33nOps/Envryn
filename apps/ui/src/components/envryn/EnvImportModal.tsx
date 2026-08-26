import * as React from "react";
import { Eye, EyeOff, Sparkles } from "lucide-react";
import { toast } from "sonner";
import { Button, Field, Input, Modal, Select } from "@/components/envryn/ui";
import { secretTypes, type Environment, type SecretType } from "@/lib/envryn-data";
import { useCreateSecret, useProjects } from "@/lib/use-vault";
import { KIND_TO_TYPE } from "@/lib/vault-repository";
import * as ipc from "@/lib/ipc";

const ENVIRONMENTS: Environment[] = ["Development", "Staging", "Production", "—"];

interface ParsedEntry {
  key: string;
  value: string;
  type: SecretType;
  include: boolean;
  revealed: boolean;
}

/**
 * `.env` line grammar: optional leading `export `, a bare or quoted value,
 * `#` comments, and blank lines. Deliberately not a full parser -- a `.env`
 * file with genuinely exotic syntax (multi-line values, `${VAR}` expansion)
 * is rare enough that silently skipping the line it can't parse is safer
 * than guessing at it.
 */
export function parseEnvText(text: string): { key: string; value: string }[] {
  const entries: { key: string; value: string }[] = [];
  for (const rawLine of text.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (!line || line.startsWith("#")) continue;
    const match = /^(?:export\s+)?([A-Za-z_]\w*)\s*=\s*(.*)$/.exec(line);
    if (!match?.[1] || match[2] === undefined) continue;
    const key = match[1];
    let value = match[2].trim();
    if (
      (value.startsWith('"') && value.endsWith('"') && value.length >= 2) ||
      (value.startsWith("'") && value.endsWith("'") && value.length >= 2)
    ) {
      value = value.slice(1, -1);
    }
    if (!key || !value) continue;
    entries.push({ key, value });
  }
  return entries;
}

/** Deterministic classification for every parsed entry; returns the keys it couldn't place. */
async function classifyDeterministically(draft: ParsedEntry[]): Promise<string[]> {
  const undetected: string[] = [];
  for (const entry of draft) {
    const deterministic = await ipc.classifyDeterministic(entry.value).catch(() => null);
    if (deterministic) {
      entry.type = KIND_TO_TYPE[deterministic.kind];
    } else {
      undetected.push(entry.key);
    }
  }
  return undetected;
}

/** Local AI fallback for names deterministic matching couldn't place -- a no-op if AI is off. */
async function classifyRemainingWithAi(draft: ParsedEntry[], undetected: string[]): Promise<void> {
  if (undetected.length === 0) return;
  const status = await ipc.aiStatus().catch(() => null);
  if (!status?.enabled_in_settings || !status.engine_running) return;
  const result = await ipc.aiClassifyEnvNames(undetected).catch(() => null);
  if (!result) return;
  const byName = new Map(result.names.map((n) => [n.name, n.kind]));
  for (const entry of draft) {
    const kind = byName.get(entry.key);
    if (kind) entry.type = KIND_TO_TYPE[kind];
  }
}

function envImportDescription(stage: "paste" | "review", entryCount: number): string {
  if (stage === "paste") return "Paste the contents below. Nothing leaves this device.";
  return `Review ${entryCount} variable${entryCount === 1 ? "" : "s"} before saving.`;
}

export function EnvImportModal({
  open,
  onOpenChange,
}: Readonly<{
  open: boolean;
  onOpenChange: (v: boolean) => void;
}>) {
  const projects = useProjects();
  const createSecret = useCreateSecret();

  const [stage, setStage] = React.useState<"paste" | "review">("paste");
  const [text, setText] = React.useState("");
  const [project, setProject] = React.useState("");
  const [environment, setEnvironment] = React.useState<Environment>("Development");
  const [entries, setEntries] = React.useState<ParsedEntry[]>([]);
  const [classifying, setClassifying] = React.useState(false);
  const [importing, setImporting] = React.useState(false);

  React.useEffect(() => {
    if (!open) return;
    setStage("paste");
    setText("");
    setProject("");
    setEnvironment("Development");
    setEntries([]);
    setImporting(false);
  }, [open]);

  /**
   * Deterministic classification first (works with no AI, and is the only
   * path that ever sees the *value*). Only names deterministic matching
   * couldn't place are sent to the local model, one batch call for the
   * whole file rather than one call per line -- and only the bare variable
   * names cross that boundary, never the values (`docs/AI_DATA_ACCESS.md`
   * Tier 1 "naming"), matching `ai_classify_env_names`'s own contract.
   */
  async function parseAndClassify() {
    const parsed = parseEnvText(text);
    if (parsed.length === 0) {
      toast("No KEY=VALUE lines were found in that text.");
      return;
    }

    const draft: ParsedEntry[] = parsed.map(({ key, value }) => ({
      key,
      value,
      type: "Environment",
      include: true,
      revealed: false,
    }));

    setClassifying(true);
    try {
      const undetected = await classifyDeterministically(draft);
      await classifyRemainingWithAi(draft, undetected);
    } finally {
      setClassifying(false);
    }

    setEntries(draft);
    setStage("review");
  }

  function toggleInclude(index: number) {
    setEntries((prev) =>
      prev.map((entry, i) => (i === index ? { ...entry, include: !entry.include } : entry)),
    );
  }

  function toggleReveal(index: number) {
    setEntries((prev) =>
      prev.map((entry, i) => (i === index ? { ...entry, revealed: !entry.revealed } : entry)),
    );
  }

  function setType(index: number, type: SecretType) {
    setEntries((prev) => prev.map((entry, i) => (i === index ? { ...entry, type } : entry)));
  }

  async function runImport() {
    const selected = entries.filter((entry) => entry.include);
    if (selected.length === 0) {
      toast("Select at least one variable to import.");
      return;
    }
    if (!project.trim()) {
      toast("Name a project for these secrets.");
      return;
    }

    setImporting(true);
    let succeeded = 0;
    const failedKeys: string[] = [];
    for (const entry of selected) {
      try {
        await createSecret.mutateAsync({
          name: entry.key,
          project: project.trim(),
          environment,
          type: entry.type,
          value: entry.value,
          notes: "",
          tags: ["imported"],
        });
        succeeded += 1;
      } catch {
        failedKeys.push(entry.key);
      }
    }
    setImporting(false);

    if (failedKeys.length === 0) {
      toast(`Imported ${succeeded} secret${succeeded === 1 ? "" : "s"}.`);
      onOpenChange(false);
    } else {
      toast(`Imported ${succeeded} of ${selected.length}. Failed: ${failedKeys.join(", ")}.`);
    }
  }

  const includedCount = entries.filter((e) => e.include).length;

  return (
    <Modal
      open={open}
      onOpenChange={onOpenChange}
      width={stage === "review" ? "sm:max-w-[640px]" : "sm:max-w-[460px]"}
      title="Import a .env file"
      description={envImportDescription(stage, entries.length)}
      footer={
        stage === "paste" ? (
          <>
            <Button onClick={() => onOpenChange(false)}>Cancel</Button>
            <Button variant="primary" loading={classifying} onClick={() => void parseAndClassify()}>
              {classifying ? "Detecting types..." : "Continue"}
            </Button>
          </>
        ) : (
          <>
            <Button onClick={() => setStage("paste")}>Back</Button>
            <Button variant="primary" loading={importing} onClick={() => void runImport()}>
              Import {includedCount} secret{includedCount === 1 ? "" : "s"}
            </Button>
          </>
        )
      }
    >
      {stage === "paste" ? (
        <div className="space-y-4">
          <Field
            label=".env contents"
            hint="One KEY=VALUE per line. Comments and blank lines are skipped."
          >
            <textarea
              autoFocus
              rows={10}
              value={text}
              onChange={(event) => setText(event.target.value)}
              className="w-full rounded-md border border-input bg-surface px-2 py-1.5 font-mono text-[12px] placeholder:text-subtle-foreground focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/25"
              placeholder={"DATABASE_URL=postgres://...\nSTRIPE_SECRET_KEY=sk_live_..."}
            />
          </Field>
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
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
        </div>
      ) : (
        <div className="space-y-3">
          <div className="flex items-center gap-1.5 text-[10.5px] text-subtle-foreground">
            <Sparkles className="size-3" />
            Types were detected automatically -- check them before importing.
          </div>
          <div className="max-h-[360px] overflow-y-auto rounded-md border border-border">
            <table className="w-full text-left text-[12px]">
              <thead className="sticky top-0 bg-surface-2 text-[10.5px] uppercase tracking-wide text-subtle-foreground">
                <tr>
                  <th className="w-8 px-2.5 py-1.5"></th>
                  <th className="px-2.5 py-1.5">Name</th>
                  <th className="px-2.5 py-1.5">Value</th>
                  <th className="w-[140px] px-2.5 py-1.5">Type</th>
                </tr>
              </thead>
              <tbody>
                {entries.map((entry, index) => (
                  <tr key={`${entry.key}-${index}`} className="border-t border-border/70">
                    <td className="px-2.5 py-1.5">
                      <input
                        type="checkbox"
                        checked={entry.include}
                        onChange={() => toggleInclude(index)}
                        aria-label={`Import ${entry.key}`}
                      />
                    </td>
                    <td className="px-2.5 py-1.5 font-mono">{entry.key}</td>
                    <td className="px-2.5 py-1.5">
                      <div className="flex items-center gap-1.5">
                        <span className="truncate font-mono text-muted-foreground">
                          {entry.revealed
                            ? entry.value
                            : "•".repeat(Math.min(entry.value.length, 16))}
                        </span>
                        <button
                          type="button"
                          onClick={() => toggleReveal(index)}
                          aria-label={entry.revealed ? "Hide value" : "Show value"}
                          className="inline-flex size-5 shrink-0 items-center justify-center rounded text-subtle-foreground hover:bg-surface-3 hover:text-foreground"
                        >
                          {entry.revealed ? (
                            <EyeOff className="size-3" />
                          ) : (
                            <Eye className="size-3" />
                          )}
                        </button>
                      </div>
                    </td>
                    <td className="px-2.5 py-1.5">
                      <Select
                        value={entry.type}
                        onChange={(event) => setType(index, event.target.value as SecretType)}
                        className="h-6.5 text-[11.5px]"
                      >
                        {secretTypes.map((item) => (
                          <option key={item}>{item}</option>
                        ))}
                      </Select>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}
    </Modal>
  );
}
