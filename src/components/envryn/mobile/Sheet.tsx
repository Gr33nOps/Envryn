import * as React from "react";
import * as DialogPrimitive from "@radix-ui/react-dialog";
import { cn } from "@/lib/utils";

/** Bottom sheet used across the mobile app. */
export function Sheet({
  open,
  onOpenChange,
  title,
  description,
  children,
  footer,
  full,
}: {
  open: boolean;
  onOpenChange: (v: boolean) => void;
  title?: string;
  description?: string;
  children?: React.ReactNode;
  footer?: React.ReactNode;
  full?: boolean;
}) {
  return (
    <DialogPrimitive.Root open={open} onOpenChange={onOpenChange}>
      <DialogPrimitive.Portal>
        <DialogPrimitive.Overlay className="fixed inset-0 z-50 bg-black/60 data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=open]:fade-in data-[state=closed]:fade-out" />
        <DialogPrimitive.Content
          className={cn(
            "fixed inset-x-0 bottom-0 z-50 flex flex-col rounded-t-2xl border-t border-border bg-surface shadow-2xl outline-none",
            "data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=open]:slide-in-from-bottom data-[state=closed]:slide-out-to-bottom duration-200",
            full ? "top-8" : "max-h-[86vh]",
          )}
        >
          <div className="flex justify-center pt-2">
            <span className="h-1 w-9 rounded-full bg-surface-3" />
          </div>
          {title && (
            <div className="px-4 pb-2 pt-3">
              <DialogPrimitive.Title className="text-[15px] font-semibold tracking-[-0.01em]">
                {title}
              </DialogPrimitive.Title>
              {description && (
                <DialogPrimitive.Description className="mt-0.5 text-[12.5px] text-muted-foreground">
                  {description}
                </DialogPrimitive.Description>
              )}
            </div>
          )}
          <div className="min-h-0 flex-1 overflow-y-auto px-4 pb-4">{children}</div>
          {footer && (
            <div className="flex gap-2 border-t border-border px-4 py-3 [&>*]:flex-1">
              {footer}
            </div>
          )}
        </DialogPrimitive.Content>
      </DialogPrimitive.Portal>
    </DialogPrimitive.Root>
  );
}

/** Large touch-friendly button for mobile. */
export function TouchButton({
  className,
  variant = "secondary",
  children,
  ...props
}: React.ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: "primary" | "secondary" | "ghost" | "danger";
}) {
  return (
    <button
      className={cn(
        "inline-flex h-11 select-none items-center justify-center gap-2 rounded-xl border px-4 text-[13.5px] font-medium transition-colors active:scale-[0.985] [&_svg]:size-4 [&_svg]:shrink-0",
        variant === "primary" &&
          "border-transparent bg-primary text-primary-foreground",
        variant === "secondary" && "border-border bg-surface-2 text-foreground",
        variant === "ghost" &&
          "border-transparent bg-transparent text-muted-foreground",
        variant === "danger" &&
          "border-destructive/40 bg-destructive-muted text-destructive",
        className,
      )}
      {...props}
    >
      {children}
    </button>
  );
}

export function MobileInput({
  className,
  mono,
  ...props
}: React.InputHTMLAttributes<HTMLInputElement> & { mono?: boolean }) {
  return (
    <input
      className={cn(
        "h-11 w-full rounded-xl border border-input bg-background px-3 text-[13.5px] text-foreground placeholder:text-subtle-foreground focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/25",
        mono && "font-mono text-[12.5px]",
        className,
      )}
      {...props}
    />
  );
}

export function MobileField({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: React.ReactNode;
}) {
  return (
    <div className="space-y-1.5">
      <label className="block text-[11.5px] font-medium uppercase tracking-[0.06em] text-subtle-foreground">
        {label}
      </label>
      {children}
      {hint && <p className="text-[11.5px] text-subtle-foreground">{hint}</p>}
    </div>
  );
}

export function ListCard({
  children,
  className,
}: {
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <div
      className={cn(
        "overflow-hidden rounded-xl border border-border bg-surface",
        className,
      )}
    >
      {children}
    </div>
  );
}
