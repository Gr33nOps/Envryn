import { cn } from "@/lib/utils";
import { estimatePasswordStrength } from "@/lib/password-strength";

const SEGMENT_COLOR = [
  "bg-destructive",
  "bg-destructive",
  "bg-warning",
  "bg-primary",
  "bg-success",
] as const;

/**
 * Local, offline strength feedback for a password field -- see
 * `lib/password-strength.ts` for what it does and does not measure. Renders
 * nothing while the field is empty, so it never implies a judgement about a
 * password the user has not started typing yet.
 */
export function PasswordStrengthMeter({ password }: Readonly<{ password: string }>) {
  const result = estimatePasswordStrength(password);
  if (password.length === 0) return null;

  return (
    <div className="space-y-1">
      <div
        className="flex gap-1"
        role="img"
        aria-label={`Estimated password strength: ${result.label}`}
      >
        {[0, 1, 2, 3].map((segment) => (
          <div
            key={segment}
            className={cn(
              "h-1 flex-1 rounded-full bg-border transition-colors",
              segment <= result.score && SEGMENT_COLOR[result.score],
            )}
          />
        ))}
      </div>
      <p className="text-[11px] text-subtle-foreground">
        {result.label}
        {result.suggestions.length > 0 ? ` -- ${result.suggestions[0]}` : ""}
      </p>
    </div>
  );
}
