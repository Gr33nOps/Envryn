import * as React from "react";
import * as DialogPrimitive from "@radix-ui/react-dialog";
import * as SwitchPrimitive from "@radix-ui/react-switch";
import * as TooltipPrimitive from "@radix-ui/react-tooltip";
import { cva, type VariantProps } from "class-variance-authority";
import { X, Search, ChevronDown } from "lucide-react";
import { cn } from "@/lib/utils";

/* ------------------------------------------------------------------ Button */

const buttonVariants = cva(
  "inline-flex select-none items-center justify-center gap-1.5 whitespace-nowrap rounded-md border text-[12.5px] font-medium transition-colors duration-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/45 disabled:pointer-events-none disabled:opacity-40 active:translate-y-px [&_svg]:size-3.5 [&_svg]:shrink-0",
  {
    variants: {
      variant: {
        primary:
          "border-transparent bg-primary text-primary-foreground hover:bg-[var(--primary-hover)] active:bg-[var(--primary-active)]",
        secondary:
          "border-border bg-surface text-foreground hover:bg-surface-2 hover:border-border-strong",
        ghost:
          "border-transparent bg-transparent text-muted-foreground hover:bg-surface-2 hover:text-foreground",
        danger:
          "border-destructive/40 bg-destructive-muted text-destructive hover:bg-destructive hover:text-destructive-foreground hover:border-destructive",
        link: "border-transparent bg-transparent text-primary hover:underline px-0",
      },
      size: {
        sm: "h-6 px-2",
        md: "h-7 px-2.5",
        lg: "h-8 px-3",
        block: "h-8 w-full px-3",
      },
    },
    defaultVariants: { variant: "secondary", size: "md" },
  },
);

export interface ButtonProps
  extends React.ButtonHTMLAttributes<HTMLButtonElement>, VariantProps<typeof buttonVariants> {
  loading?: boolean;
}

export function Button({
  className,
  variant,
  size,
  loading,
  children,
  disabled,
  ...props
}: ButtonProps) {
  return (
    <button
      className={cn(buttonVariants({ variant, size }), className)}
      disabled={disabled || loading}
      {...props}
    >
      {loading && (
        <span className="size-3 animate-spin rounded-full border-[1.5px] border-current border-t-transparent" />
      )}
      {children}
    </button>
  );
}

export function IconButton({
  className,
  label,
  children,
  ...props
}: React.ButtonHTMLAttributes<HTMLButtonElement> & { label: string }) {
  return (
    <Tooltip content={label}>
      <button
        aria-label={label}
        className={cn(
          "inline-flex size-6 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-surface-3 hover:text-foreground [&_svg]:size-3.5",
          className,
        )}
        {...props}
      >
        {children}
      </button>
    </Tooltip>
  );
}

/* ------------------------------------------------------------------- Input */

export const Input = React.forwardRef<
  HTMLInputElement,
  React.InputHTMLAttributes<HTMLInputElement> & { invalid?: boolean; mono?: boolean }
>(function Input({ className, invalid, mono, ...props }, ref) {
  return (
    <input
      ref={ref}
      aria-invalid={invalid || undefined}
      className={cn(
        "h-7 w-full rounded-md border border-input bg-surface px-2 text-[12.5px] text-foreground transition-colors placeholder:text-subtle-foreground hover:border-border-strong focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/25 disabled:opacity-45",
        mono && "font-mono text-[12px] tracking-tight",
        invalid && "border-destructive focus:border-destructive focus:ring-destructive/25",
        className,
      )}
      {...props}
    />
  );
});

export function Field({
  label,
  hint,
  error,
  children,
  className,
}: {
  label: string;
  hint?: string | undefined;
  error?: string | undefined;
  children: React.ReactNode;
  className?: string | undefined;
}) {
  return (
    <div className={cn("space-y-1", className)}>
      <label className="block text-[11px] font-medium text-muted-foreground">{label}</label>
      {children}
      {error ? (
        <p className="text-[11px] text-destructive">{error}</p>
      ) : hint ? (
        <p className="text-[11px] text-subtle-foreground">{hint}</p>
      ) : null}
    </div>
  );
}

export function SearchField({
  className,
  shortcut,
  ...props
}: React.InputHTMLAttributes<HTMLInputElement> & { shortcut?: string }) {
  return (
    <div className={cn("relative", className)}>
      <Search className="pointer-events-none absolute left-2 top-1/2 size-3.5 -translate-y-1/2 text-subtle-foreground" />
      <input
        className="h-7 w-full rounded-md border border-input bg-surface pl-7 pr-14 text-[12.5px] transition-colors placeholder:text-subtle-foreground hover:border-border-strong focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/25"
        {...props}
      />
      {shortcut && (
        <span className="pointer-events-none absolute right-2 top-1/2 -translate-y-1/2 kbd">
          {shortcut}
        </span>
      )}
    </div>
  );
}

export function Select({
  className,
  children,
  ...props
}: React.SelectHTMLAttributes<HTMLSelectElement>) {
  return (
    <div className="relative">
      <select
        className={cn(
          "h-7 w-full appearance-none rounded-md border border-input bg-surface px-2 pr-7 text-[12.5px] text-foreground transition-colors hover:border-border-strong focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/25",
          className,
        )}
        {...props}
      >
        {children}
      </select>
      <ChevronDown className="pointer-events-none absolute right-2 top-1/2 size-3.5 -translate-y-1/2 text-subtle-foreground" />
    </div>
  );
}

export function Switch({
  checked,
  onCheckedChange,
}: {
  checked: boolean;
  onCheckedChange: (v: boolean) => void;
}) {
  return (
    <SwitchPrimitive.Root
      checked={checked}
      onCheckedChange={onCheckedChange}
      className="h-4 w-7 shrink-0 rounded-full border border-border bg-surface-3 transition-colors data-[state=checked]:border-transparent data-[state=checked]:bg-primary"
    >
      <SwitchPrimitive.Thumb className="block size-3 translate-x-0.5 rounded-full bg-foreground/80 transition-transform data-[state=checked]:translate-x-3.5 data-[state=checked]:bg-primary-foreground" />
    </SwitchPrimitive.Root>
  );
}

/* -------------------------------------------------------------------- Tabs */

export function Tabs({
  items,
  value,
  onChange,
  variant = "underline",
  className,
}: {
  items: { value: string; label: string; count?: number }[];
  value: string;
  onChange: (v: string) => void;
  variant?: "underline" | "segmented";
  className?: string;
}) {
  if (variant === "segmented") {
    return (
      <div
        className={cn(
          "inline-flex items-center gap-0.5 rounded-md border border-border bg-surface p-0.5",
          className,
        )}
      >
        {items.map((i) => (
          <button
            key={i.value}
            onClick={() => onChange(i.value)}
            aria-pressed={value === i.value}
            className={cn(
              "h-6 rounded-[4px] px-2.5 text-[12px] font-medium transition-colors",
              value === i.value
                ? "border border-border-strong bg-surface-3 text-foreground"
                : "text-muted-foreground hover:bg-surface-2 hover:text-foreground",
            )}
          >
            {i.label}
            {i.count !== undefined && <span className="ml-1.5 opacity-60">{i.count}</span>}
          </button>
        ))}
      </div>
    );
  }
  return (
    <div className={cn("flex items-center gap-4 border-b border-border", className)}>
      {items.map((i) => (
        <button
          key={i.value}
          onClick={() => onChange(i.value)}
          className={cn(
            "-mb-px border-b py-1.5 text-[12.5px] transition-colors",
            value === i.value
              ? "border-primary font-medium text-foreground"
              : "border-transparent text-muted-foreground hover:text-foreground",
          )}
        >
          {i.label}
          {i.count !== undefined && (
            <span className="ml-1.5 text-subtle-foreground">{i.count}</span>
          )}
        </button>
      ))}
    </div>
  );
}

/* ------------------------------------------------------------------ Status */

export function StatusDot({
  tone = "neutral",
  className,
}: {
  tone?: "success" | "warning" | "danger" | "neutral" | "syncing";
  className?: string;
}) {
  return (
    <span
      className={cn(
        "inline-block size-1.5 shrink-0 rounded-full",
        tone === "success" && "bg-success",
        tone === "warning" && "bg-warning",
        tone === "danger" && "bg-destructive",
        tone === "neutral" && "bg-subtle-foreground",
        tone === "syncing" && "animate-pulse bg-primary",
        className,
      )}
    />
  );
}

export function StatusLabel({
  tone,
  children,
}: {
  tone: "success" | "warning" | "danger" | "neutral" | "syncing";
  children: React.ReactNode;
}) {
  return (
    <span
      className={cn(
        "inline-flex items-center gap-1.5 text-[11.5px]",
        tone === "success" && "text-success",
        tone === "warning" && "text-warning",
        tone === "danger" && "text-destructive",
        tone === "syncing" && "text-primary",
        tone === "neutral" && "text-muted-foreground",
      )}
    >
      <StatusDot tone={tone} />
      {children}
    </span>
  );
}

export function TypeTag({ type }: { type: string }) {
  return <span className="text-[11.5px] text-muted-foreground">{type}</span>;
}

/* ------------------------------------------------------------------- Modal */

export function Modal({
  open,
  onOpenChange,
  title,
  description,
  children,
  footer,
  width = "sm:max-w-[420px]",
}: {
  open: boolean;
  onOpenChange: (v: boolean) => void;
  title: string;
  description?: string;
  children?: React.ReactNode;
  footer?: React.ReactNode;
  width?: string;
}) {
  return (
    <DialogPrimitive.Root open={open} onOpenChange={onOpenChange}>
      <DialogPrimitive.Portal>
        <DialogPrimitive.Overlay className="fixed inset-0 z-50 bg-black/55 data-[state=open]:animate-in data-[state=open]:fade-in-0" />
        <DialogPrimitive.Content
          className={cn(
            "fixed left-1/2 top-1/2 z-50 w-full -translate-x-1/2 -translate-y-1/2 rounded-lg border border-border bg-surface shadow-[0_16px_48px_-12px_rgba(0,0,0,0.6)] data-[state=open]:animate-in data-[state=open]:fade-in-0 data-[state=open]:zoom-in-98",
            width,
          )}
        >
          <div className="flex items-start justify-between gap-4 border-b border-border px-4 py-2.5">
            <div>
              <DialogPrimitive.Title className="text-[13px] font-medium">
                {title}
              </DialogPrimitive.Title>
              {description && (
                <DialogPrimitive.Description className="mt-0.5 max-w-[46ch] text-[12px] leading-snug text-muted-foreground">
                  {description}
                </DialogPrimitive.Description>
              )}
            </div>
            <DialogPrimitive.Close asChild>
              <button
                aria-label="Close"
                className="-mr-1 mt-0.5 inline-flex size-5 items-center justify-center rounded text-subtle-foreground hover:bg-surface-3 hover:text-foreground"
              >
                <X className="size-3.5" />
              </button>
            </DialogPrimitive.Close>
          </div>
          {children && <div className="px-4 py-3.5">{children}</div>}
          {footer && (
            <div className="flex items-center justify-end gap-2 border-t border-border px-4 py-2.5">
              {footer}
            </div>
          )}
        </DialogPrimitive.Content>
      </DialogPrimitive.Portal>
    </DialogPrimitive.Root>
  );
}

export function ConfirmDialog({
  open,
  onOpenChange,
  title,
  body,
  confirmLabel,
  onConfirm,
  destructive = true,
}: {
  open: boolean;
  onOpenChange: (v: boolean) => void;
  title: string;
  body: string;
  confirmLabel: string;
  onConfirm: () => void;
  destructive?: boolean;
}) {
  return (
    <Modal
      open={open}
      onOpenChange={onOpenChange}
      title={title}
      description={body}
      footer={
        <>
          <Button onClick={() => onOpenChange(false)}>Cancel</Button>
          <Button
            variant={destructive ? "danger" : "primary"}
            onClick={() => {
              onConfirm();
              onOpenChange(false);
            }}
          >
            {confirmLabel}
          </Button>
        </>
      }
    />
  );
}

/* ----------------------------------------------------------------- Tooltip */

export function Tooltip({
  content,
  children,
  side = "top",
}: {
  content: string;
  children: React.ReactNode;
  side?: "top" | "bottom" | "left" | "right";
}) {
  return (
    <TooltipPrimitive.Provider delayDuration={350}>
      <TooltipPrimitive.Root>
        <TooltipPrimitive.Trigger asChild>{children}</TooltipPrimitive.Trigger>
        <TooltipPrimitive.Portal>
          <TooltipPrimitive.Content
            side={side}
            sideOffset={5}
            className="z-50 rounded border border-border bg-surface-2 px-1.5 py-0.5 text-[11px] text-foreground shadow-lg"
          >
            {content}
          </TooltipPrimitive.Content>
        </TooltipPrimitive.Portal>
      </TooltipPrimitive.Root>
    </TooltipPrimitive.Provider>
  );
}

/* -------------------------------------------------------------- Structural */

export function PageHeader({
  title,
  subtitle,
  actions,
  back,
}: {
  title: React.ReactNode;
  subtitle?: React.ReactNode;
  actions?: React.ReactNode;
  back?: React.ReactNode;
}) {
  return (
    <div className="flex items-start justify-between gap-4 px-5 pb-3 pt-4">
      <div className="min-w-0">
        {back}
        <h1 className="truncate text-[15px] font-semibold tracking-[-0.01em]">{title}</h1>
        {subtitle && <p className="mt-0.5 text-[12px] text-muted-foreground">{subtitle}</p>}
      </div>
      {actions && <div className="flex shrink-0 items-center gap-2">{actions}</div>}
    </div>
  );
}

export function SectionLabel({ children }: { children: React.ReactNode }) {
  return (
    <div className="text-[10.5px] font-medium uppercase tracking-[0.08em] text-subtle-foreground">
      {children}
    </div>
  );
}

export function EmptyState({
  title,
  body,
  action,
}: {
  title: string;
  body?: string;
  action?: React.ReactNode;
}) {
  return (
    <div className="flex flex-col items-center justify-center px-6 py-16 text-center">
      <p className="text-[13px] font-medium text-foreground">{title}</p>
      {body && (
        <p className="mt-1 max-w-[38ch] text-[12px] leading-relaxed text-muted-foreground">
          {body}
        </p>
      )}
      {action && <div className="mt-3.5">{action}</div>}
    </div>
  );
}

export function SettingsRow({
  label,
  description,
  control,
}: {
  label: string;
  description?: string;
  control: React.ReactNode;
}) {
  return (
    <div className="flex items-center justify-between gap-6 border-b border-border/70 px-3 py-2 last:border-0">
      <div className="min-w-0">
        <p className="text-[12.5px]">{label}</p>
        {description && (
          <p className="mt-0.5 text-[11.5px] text-subtle-foreground">{description}</p>
        )}
      </div>
      <div className="shrink-0">{control}</div>
    </div>
  );
}

export function Panel({ children, className }: { children: React.ReactNode; className?: string }) {
  return (
    <div className={cn("overflow-hidden rounded-md border border-border bg-surface", className)}>
      {children}
    </div>
  );
}

export function DetailRow({
  label,
  value,
  mono,
}: {
  label: string;
  value: React.ReactNode;
  mono?: boolean;
}) {
  return (
    <div>
      <div className="text-[10.5px] font-medium uppercase tracking-[0.08em] text-subtle-foreground">
        {label}
      </div>
      <div className={cn("mt-0.5 text-[12.5px]", mono && "font-mono text-[12px]")}>{value}</div>
    </div>
  );
}
