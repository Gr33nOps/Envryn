import * as React from "react";
import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { Eye, EyeOff, KeyRound, ShieldCheck } from "lucide-react";
import { Button, IconButton, Input } from "@/components/envryn/ui";
import { LogoMark } from "@/components/envryn/Logo";
import { IpcError, vaultCreate, vaultStatus, vaultUnlock } from "@/lib/ipc";

export const Route = createFileRoute("/")({
  component: Unlock,
});

/** Which form to show: unlocking an existing vault, or creating the first one. */
type Mode = "loading" | "unlock" | "create";

function Unlock() {
  const navigate = useNavigate();
  const [mode, setMode] = React.useState<Mode>("loading");
  const [value, setValue] = React.useState("");
  const [confirm, setConfirm] = React.useState("");
  const [error, setError] = React.useState<string | null>(null);
  const [loading, setLoading] = React.useState(false);
  const [showPassword, setShowPassword] = React.useState(false);

  React.useEffect(() => {
    let cancelled = false;
    vaultStatus()
      .then((status) => {
        if (!cancelled) setMode(status.exists ? "unlock" : "create");
      })
      .catch((err) => {
        if (cancelled) return;
        setMode("unlock");
        setError(err instanceof IpcError ? err.message : "Envryn could not start.");
      });
    return () => {
      cancelled = true;
    };
  }, []);

  async function submit(event: React.FormEvent) {
    event.preventDefault();
    setError(null);

    if (mode === "create" && value !== confirm) {
      setError("Those passwords do not match.");
      return;
    }

    setLoading(true);
    try {
      if (mode === "create") {
        await vaultCreate(value);
      } else {
        await vaultUnlock(value);
      }
      // Clear the password from React state the moment it is no longer needed.
      setValue("");
      setConfirm("");
      await navigate({ to: "/vault" });
    } catch (err) {
      setError(
        err instanceof IpcError
          ? err.code === "auth_failed"
            ? "That password did not work. Please try again."
            : err.message
          : "Something went wrong. Your vault is unaffected.",
      );
    } finally {
      setLoading(false);
    }
  }

  const creating = mode === "create";

  return (
    <main className="unlock-page flex min-h-screen items-center justify-center bg-background px-5 py-10">
      <form onSubmit={submit} className="w-full max-w-[420px]">
        <div className="flex items-center gap-3 px-1">
          <LogoMark size={34} />
          <div>
            <h1 className="text-[17px] font-semibold tracking-[-0.025em]">
              {creating ? "Set up Envryn" : "Unlock Envryn"}
            </h1>
            <p className="mt-0.5 text-[12px] text-muted-foreground">
              Your private vault on this PC
            </p>
          </div>
        </div>

        <div className="mt-7 rounded-lg border border-border bg-surface p-5 shadow-[0_18px_60px_-36px_rgba(0,0,0,0.9)]">
          <div className="flex items-start gap-3">
            <span className="inline-flex size-9 shrink-0 items-center justify-center rounded-md border border-border bg-surface-2 text-muted-foreground">
              <KeyRound className="size-4" />
            </span>
            <div>
              <p className="text-[13px] font-medium">
                {creating ? "Choose a master password" : "Enter your master password"}
              </p>
              <p className="mt-1 text-[12px] leading-relaxed text-muted-foreground">
                {creating
                  ? "This is the only way into your vault. Envryn cannot reset it for you, because it never leaves this computer."
                  : "This opens the secrets stored in your vault. The password is only used on this computer."}
              </p>
            </div>
          </div>

          <div className="mt-5 space-y-2.5">
            <div className="relative">
              <Input
                autoFocus
                type={showPassword ? "text" : "password"}
                aria-label="Master password"
                placeholder="Master password"
                className="h-9 pr-9"
                invalid={Boolean(error)}
                disabled={mode === "loading"}
                value={value}
                onChange={(event) => {
                  setValue(event.target.value);
                  setError(null);
                }}
              />
              <IconButton
                label={showPassword ? "Hide password" : "Show password"}
                className="absolute right-1 top-1/2 -translate-y-1/2"
                onClick={() => setShowPassword((visible) => !visible)}
              >
                {showPassword ? <EyeOff /> : <Eye />}
              </IconButton>
            </div>

            {creating && (
              <Input
                type={showPassword ? "text" : "password"}
                aria-label="Confirm master password"
                placeholder="Confirm master password"
                className="h-9"
                invalid={Boolean(error)}
                value={confirm}
                onChange={(event) => {
                  setConfirm(event.target.value);
                  setError(null);
                }}
              />
            )}

            {error && <p className="text-[12px] text-destructive">{error}</p>}

            <Button
              type="submit"
              variant="primary"
              size="block"
              loading={loading}
              disabled={mode === "loading" || value.length === 0}
            >
              {creating ? "Create vault" : "Unlock vault"}
            </Button>
          </div>
        </div>

        <div className="mt-4 flex items-start gap-2.5 px-1 text-[12px] leading-relaxed text-muted-foreground">
          <ShieldCheck className="mt-0.5 size-3.5 shrink-0 text-success" />
          <p>Secrets stay on this computer. Nothing is sent to an online account.</p>
        </div>
      </form>
    </main>
  );
}
