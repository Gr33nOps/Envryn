import * as React from "react";
import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { Eye, EyeOff, KeyRound, ShieldCheck } from "lucide-react";
import { toast } from "sonner";
import { Button, IconButton, Input } from "@/components/envryn/ui";
import { LogoMark } from "@/components/envryn/Logo";

export const Route = createFileRoute("/")({
  component: Unlock,
});

function Unlock() {
  const navigate = useNavigate();
  const [value, setValue] = React.useState("");
  const [error, setError] = React.useState(false);
  const [loading, setLoading] = React.useState(false);
  const [showPassword, setShowPassword] = React.useState(false);

  function submit(event: React.FormEvent) {
    event.preventDefault();
    if (value.trim().toLowerCase() === "wrong") return setError(true);
    setLoading(true);
    setTimeout(() => navigate({ to: "/vault" }), 400);
  }

  return (
    <main className="unlock-page flex min-h-screen items-center justify-center bg-background px-5 py-10">
      <form onSubmit={submit} className="w-full max-w-[420px]">
        <div className="flex items-center gap-3 px-1">
          <LogoMark size={34} />
          <div>
            <h1 className="text-[17px] font-semibold tracking-[-0.025em]">Unlock Envryn</h1>
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
              <p className="text-[13px] font-medium">Enter your master password</p>
              <p className="mt-1 text-[12px] leading-relaxed text-muted-foreground">
                This opens the secrets stored in your vault. The password is only used on this
                computer.
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
                invalid={error}
                value={value}
                onChange={(event) => {
                  setValue(event.target.value);
                  setError(false);
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
            {error && (
              <p className="text-[12px] text-destructive">
                That password did not work. Please try again.
              </p>
            )}
            <Button type="submit" variant="primary" size="block" loading={loading}>
              Unlock vault
            </Button>
            <Button
              type="button"
              variant="secondary"
              size="block"
              onClick={() => toast("Windows Hello is ready to connect")}
            >
              Use Windows Hello
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
