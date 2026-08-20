import { AlertTriangle, ChevronRight, Copy } from "lucide-react";
import { toast } from "sonner";
import { cn } from "@/lib/utils";
import type { Secret } from "@/lib/envryn-data";
import { useMobileUI } from "./mobile-context";

export function copySecretMobile() {
  toast("Secret copied", { description: "Clipboard clears in 30 seconds." });
}

export function EnvDot({ env }: { env: string }) {
  return (
    <span
      className={cn(
        "size-1.5 shrink-0 rounded-full",
        env === "Production"
          ? "bg-warning"
          : env === "Staging"
            ? "bg-primary"
            : env === "Development"
              ? "bg-success"
              : "bg-subtle-foreground",
      )}
    />
  );
}

export function SecretRow({ secret }: { secret: Secret }) {
  const { select } = useMobileUI();
  return (
    <div
      role="button"
      tabIndex={0}
      onClick={() => select(secret)}
      onKeyDown={(e) => e.key === "Enter" && select(secret)}
      className="flex w-full items-center gap-3 border-b border-border/60 px-3 py-2.5 text-left transition-colors last:border-0 active:bg-surface-2"
    >
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-1.5">
          <span
            className={cn(
              "truncate text-[13.5px]",
              secret.name === secret.name.toUpperCase() && "font-mono text-[12.5px]",
            )}
          >
            {secret.name}
          </span>
          {secret.damaged && (
            <AlertTriangle className="size-3.5 shrink-0 text-warning" />
          )}
        </div>
        <div className="mt-0.5 flex items-center gap-1.5 text-[11.5px] text-muted-foreground">
          <span className="truncate">{secret.project}</span>
          {secret.environment !== "—" && (
            <>
              <span className="text-subtle-foreground">·</span>
              <EnvDot env={secret.environment} />
              <span className="truncate">{secret.environment}</span>
            </>
          )}
          <span className="text-subtle-foreground">·</span>
          <span className="truncate">{secret.type}</span>
        </div>
      </div>
      <button
        aria-label="Copy"
        onClick={(e) => {
          e.stopPropagation();
          copySecretMobile();
        }}
        className="grid size-9 shrink-0 place-items-center rounded-lg text-muted-foreground active:bg-surface-2"
      >
        <Copy className="size-4" />
      </button>
      <ChevronRight className="size-4 shrink-0 text-subtle-foreground" />
    </div>
  );
}
