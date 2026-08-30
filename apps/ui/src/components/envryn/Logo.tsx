import { cn } from "@/lib/utils";
import mark from "@/assets/envryn-mark.png";

/**
 * Envryn logo mark.
 * Primary treatment: green mark on OLED black, transparent background.
 * `mono` renders a white monochrome variant for contexts where green is
 * inappropriate (e.g. over the brand-green surface itself).
 */
export function LogoMark({
  size = 18,
  mono,
  className,
}: Readonly<{
  size?: number;
  mono?: boolean;
  className?: string;
}>) {
  return (
    <img
      src={mark}
      alt=""
      width={size}
      height={size}
      className={cn("shrink-0 select-none", mono && "brightness-0 invert", className)}
      style={{
        width: size,
        height: size,
        boxSizing: "border-box",
        objectFit: "contain",
        padding: size <= 28 ? 2 : 0,
      }}
    />
  );
}

export function Wordmark({
  size = 18,
  className,
  subtitle,
}: Readonly<{
  size?: number;
  className?: string;
  subtitle?: string;
}>) {
  return (
    <div className={cn("flex items-center gap-2", className)}>
      <LogoMark size={size} />
      <div className="leading-none">
        <div className="text-[13px] font-semibold tracking-[-0.015em] text-foreground">Envryn</div>
        {subtitle && (
          <div className="mt-0.5 font-mono text-[10px] tracking-tight text-subtle-foreground">
            {subtitle}
          </div>
        )}
      </div>
    </div>
  );
}
