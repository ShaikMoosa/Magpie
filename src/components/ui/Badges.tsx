import { AlertTriangle, Shield, ShieldAlert, ShieldCheck } from "lucide-react";
import clsx from "clsx";
import type { Category, SecurityStatus } from "../../lib/types";
import { UNCATEGORIZED_COLOR, tint } from "../../lib/colors";

const SECURITY: Record<SecurityStatus, { label: string; cls: string; Icon: typeof Shield }> = {
  no_threat: { label: "No threat", cls: "bg-success-bg text-success", Icon: ShieldCheck },
  suspicious: { label: "Suspicious", cls: "bg-warning-bg text-warning", Icon: ShieldAlert },
  dangerous: { label: "Dangerous", cls: "bg-danger-bg text-danger", Icon: AlertTriangle },
  not_checked: { label: "Not checked", cls: "bg-muted text-ink-2", Icon: Shield },
};

export function SecurityBadge({ status, className }: { status: SecurityStatus; className?: string }) {
  const s = SECURITY[status];
  return (
    <span className={clsx("inline-flex h-[22px] items-center gap-1 rounded-full px-2 text-xs font-medium whitespace-nowrap", s.cls, className)}>
      <s.Icon className="size-3" aria-hidden />
      {s.label}
    </span>
  );
}

export function CategoryPill({ category, onClick }: { category: Category | null | undefined; onClick?: () => void }) {
  const color = category?.color ?? UNCATEGORIZED_COLOR;
  const Comp = onClick ? "button" : "span";
  return (
    <Comp
      type={onClick ? "button" : undefined}
      onClick={(e) => {
        if (!onClick) return;
        e.stopPropagation();
        onClick();
      }}
      className="inline-flex h-[22px] max-w-40 items-center truncate rounded-full px-2 text-xs font-medium"
      style={{ background: tint(color, 0.1), color }}
    >
      <span className="truncate">{category?.name ?? "Uncategorized"}</span>
    </Comp>
  );
}

export function CountBadge({ n, active, tone }: { n: number; active?: boolean; tone?: "danger" }) {
  return (
    <span
      className={clsx(
        "rounded-full px-[7px] py-px text-xs font-medium tabular-nums",
        tone === "danger" && n > 0 ? "bg-danger-bg text-danger" : active ? "bg-brand-100 text-brand-700" : "bg-muted text-ink-3",
      )}
    >
      {n.toLocaleString()}
    </span>
  );
}
