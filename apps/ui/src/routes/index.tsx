import * as React from "react";
import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { Eye, EyeOff, KeyRound, Link2, ShieldCheck } from "lucide-react";
import { Button, Field, IconButton, Input } from "@/components/envryn/ui";
import { LogoMark } from "@/components/envryn/Logo";
import { PasswordStrengthMeter } from "@/components/envryn/PasswordStrengthMeter";
import {
  IpcError,
  listenPairingEvents,
  pairingCancel,
  pairingConfirm,
  pairingJoinStart,
  vaultCreate,
  vaultStatus,
  vaultUnlock,
  vaultUnlockWithPlatform,
  type PairingFailed,
  type PairingSasReady,
} from "@/lib/ipc";

export const Route = createFileRoute("/")({
  component: Unlock,
});

/**
 * Which form to show: unlocking an existing vault, creating the first one,
 * or joining one an already-paired device is hosting. "join" is only
 * reachable from "create" -- a device that already has a vault pairs new
 * *peers* from the Devices page instead (`apps/ui/src/routes/vault/devices.tsx`),
 * which plays the opposite (host) role in the same protocol.
 */
type Mode = "loading" | "unlock" | "create" | "join";

/** The join-existing-vault flow, extracted so `Unlock` doesn't carry its state and control flow. */
function JoinVaultView({ onCancel }: Readonly<{ onCancel: () => void }>) {
  const navigate = useNavigate();
  const [joinStage, setJoinStage] = React.useState<
    "form" | "connecting" | "confirming" | "joining" | "error"
  >("form");
  const [joinAddress, setJoinAddress] = React.useState("");
  const [joinPort, setJoinPort] = React.useState("");
  const [joinCode, setJoinCode] = React.useState("");
  const [joinPassword, setJoinPassword] = React.useState("");
  const [joinConfirmPassword, setJoinConfirmPassword] = React.useState("");
  const [joinSas, setJoinSas] = React.useState<PairingSasReady | null>(null);
  const [joinError, setJoinError] = React.useState<string | null>(null);
  const joinUnlistenRef = React.useRef<(() => void) | null>(null);

  React.useEffect(() => () => joinUnlistenRef.current?.(), []);

  async function startJoin(event: React.SubmitEvent<HTMLFormElement>) {
    event.preventDefault();
    const port = Number.parseInt(joinPort, 10);
    if (!joinAddress.trim() || Number.isNaN(port)) {
      setJoinError("Enter the address and port shown on the other device.");
      setJoinStage("error");
      return;
    }
    setJoinStage("connecting");
    setJoinError(null);
    try {
      joinUnlistenRef.current = await listenPairingEvents({
        onSasReady: (event: PairingSasReady) => {
          setJoinSas(event);
          setJoinStage("confirming");
        },
        onFailed: (event: PairingFailed) => {
          setJoinError(event.message);
          setJoinStage("error");
        },
        onComplete: () => {
          void navigate({ to: "/vault" });
        },
      });
      await pairingJoinStart(joinAddress.trim(), port, joinCode.trim() || null);
    } catch (err) {
      setJoinError(err instanceof IpcError ? err.message : "Could not reach that device.");
      setJoinStage("error");
    }
  }

  async function confirmJoin() {
    if (joinPassword.length < 8) {
      setJoinError("Choose a master password of at least 8 characters.");
      return;
    }
    if (joinPassword !== joinConfirmPassword) {
      setJoinError("Those passwords do not match.");
      return;
    }
    setJoinStage("joining");
    setJoinError(null);
    try {
      await pairingConfirm(joinPassword);
      // Outcome arrives as pairing://complete (navigates away above) or
      // pairing://failed (handled by the listener already registered).
    } catch (err) {
      setJoinError(err instanceof IpcError ? err.message : "Could not complete pairing.");
      setJoinStage("error");
    }
  }

  function cancelJoin() {
    void pairingCancel();
    joinUnlistenRef.current?.();
    onCancel();
  }

  const confirmingOrJoining = joinStage === "confirming" || joinStage === "joining";

  return (
    <main className="unlock-page flex min-h-screen items-center justify-center bg-background px-5 py-10">
      <div className="w-full max-w-[420px]">
        <div className="flex items-center gap-3 px-1">
          <LogoMark size={34} />
          <div>
            <h1 className="text-[17px] font-semibold tracking-[-0.025em]">
              Join an existing vault
            </h1>
            <p className="mt-0.5 text-[12px] text-muted-foreground">
              Pair with a device that already has one
            </p>
          </div>
        </div>

        <div className="mt-7 rounded-lg border border-border bg-surface p-5 shadow-[0_18px_60px_-36px_rgba(0,0,0,0.9)]">
          <div className="flex items-start gap-3">
            <span className="inline-flex size-9 shrink-0 items-center justify-center rounded-md border border-border bg-surface-2 text-muted-foreground">
              <Link2 className="size-4" />
            </span>
            <div>
              <p className="text-[13px] font-medium">
                {confirmingOrJoining
                  ? "Confirm and choose a password"
                  : "Connect to the other device"}
              </p>
              <p className="mt-1 text-[12px] leading-relaxed text-muted-foreground">
                {confirmingOrJoining
                  ? "Check that this code matches the one shown on the other device, then choose a master password for this device."
                  : "Open Devices → Pair a device on the other machine, then enter what it shows here."}
              </p>
            </div>
          </div>

          {confirmingOrJoining ? (
            <div className="mt-5 space-y-3">
              <div className="rounded-md border border-border bg-background px-3 py-3 text-center">
                <div className="text-[10.5px] uppercase tracking-[0.08em] text-subtle-foreground">
                  Verification code
                </div>
                <div className="mt-0.5 font-mono text-[16px] tracking-[0.2em]">
                  {joinSas?.sas ?? "······"}
                </div>
                {joinSas?.peer_fingerprint_display ? (
                  <div className="mt-1 truncate font-mono text-[10.5px] text-muted-foreground">
                    {joinSas.peer_fingerprint_display}
                  </div>
                ) : null}
              </div>
              <Field label="New master password for this device">
                <Input
                  type="password"
                  autoFocus
                  value={joinPassword}
                  onChange={(event) => {
                    setJoinPassword(event.target.value);
                    setJoinError(null);
                  }}
                />
                <PasswordStrengthMeter password={joinPassword} />
              </Field>
              <Field label="Confirm master password">
                <Input
                  type="password"
                  value={joinConfirmPassword}
                  onChange={(event) => {
                    setJoinConfirmPassword(event.target.value);
                    setJoinError(null);
                  }}
                />
              </Field>
              {joinError && <p className="text-[12px] text-destructive">{joinError}</p>}
              <Button
                type="button"
                variant="primary"
                size="block"
                loading={joinStage === "joining"}
                disabled={joinPassword.length === 0}
                onClick={() => void confirmJoin()}
              >
                Trust device &amp; join vault
              </Button>
              <Button type="button" size="block" onClick={cancelJoin}>
                Cancel
              </Button>
            </div>
          ) : (
            <form onSubmit={(event) => void startJoin(event)} className="mt-5 space-y-2.5">
              <Field label="Address">
                <Input
                  autoFocus
                  placeholder="LAN address shown on the other device"
                  value={joinAddress}
                  onChange={(event) => setJoinAddress(event.target.value)}
                />
              </Field>
              <Field label="Port">
                <Input
                  placeholder="e.g. 51820"
                  inputMode="numeric"
                  value={joinPort}
                  onChange={(event) => setJoinPort(event.target.value)}
                />
              </Field>
              <Field label="Pairing code">
                <Input
                  placeholder="6-digit code"
                  inputMode="numeric"
                  value={joinCode}
                  onChange={(event) => setJoinCode(event.target.value)}
                />
              </Field>
              {joinError && <p className="text-[12px] text-destructive">{joinError}</p>}
              <Button
                type="submit"
                variant="primary"
                size="block"
                loading={joinStage === "connecting"}
              >
                Connect
              </Button>
              <Button type="button" size="block" onClick={cancelJoin}>
                Cancel
              </Button>
            </form>
          )}
        </div>

        <div className="mt-4 flex items-start gap-2.5 px-1 text-[12px] leading-relaxed text-muted-foreground">
          <ShieldCheck className="mt-0.5 size-3.5 shrink-0 text-success" />
          <p>Devices sync directly over your local network. Nothing passes through a server.</p>
        </div>
      </div>
    </main>
  );
}

function unlockErrorMessage(err: unknown): string {
  if (!(err instanceof IpcError)) return "Something went wrong. Your vault is unaffected.";
  if (err.code === "auth_failed") return "That password did not work. Please try again.";
  return err.message;
}

function Unlock() {
  const navigate = useNavigate();
  const [mode, setMode] = React.useState<Mode>("loading");
  const [value, setValue] = React.useState("");
  const [confirm, setConfirm] = React.useState("");
  const [error, setError] = React.useState<string | null>(null);
  const [loading, setLoading] = React.useState(false);
  const [showPassword, setShowPassword] = React.useState(false);
  const [platformUnlock, setPlatformUnlock] = React.useState(false);
  const [platformLoading, setPlatformLoading] = React.useState(false);

  React.useEffect(() => {
    let cancelled = false;
    vaultStatus()
      .then((status) => {
        if (cancelled) return;
        setMode(status.exists ? "unlock" : "create");
        setPlatformUnlock(status.platform_protection_enabled);
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

  async function unlockWithPlatform() {
    setError(null);
    setPlatformLoading(true);
    try {
      await vaultUnlockWithPlatform();
      await navigate({ to: "/vault" });
    } catch (err) {
      setError(
        err instanceof IpcError
          ? "That did not work. Try your master password instead."
          : "Something went wrong. Your vault is unaffected.",
      );
    } finally {
      setPlatformLoading(false);
    }
  }

  async function submit(event: React.SubmitEvent<HTMLFormElement>) {
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
      setError(unlockErrorMessage(err));
    } finally {
      setLoading(false);
    }
  }

  const creating = mode === "create";

  if (mode === "join") {
    return <JoinVaultView onCancel={() => setMode("create")} />;
  }

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

            {creating && <PasswordStrengthMeter password={value} />}

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

            {platformUnlock && (
              <Button
                type="button"
                variant="secondary"
                size="block"
                loading={platformLoading}
                onClick={() => void unlockWithPlatform()}
              >
                Unlock with this Windows account
              </Button>
            )}

            {creating && (
              <Button type="button" variant="ghost" size="block" onClick={() => setMode("join")}>
                <Link2 />
                Join an existing vault instead
              </Button>
            )}
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
