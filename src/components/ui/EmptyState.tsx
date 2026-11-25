import clsx from "clsx";
import type { ReactNode } from "react";

type Tone = "neutral" | "brand" | "success" | "warning";

const tones: Record<Tone, string> = {
  neutral: "bg-muted text-ink-2",
  brand: "bg-brand-50 text-brand-600",
  success: "bg-success-bg text-success",
  warning: "bg-warning-bg text-warning",
};

export function EmptyState({
  icon,
  title,
  body,
  tone = "neutral",
  actions,
  className,
}: {
  icon: ReactNode;
  title: string;
  body: string;
  tone?: Tone;
  actions?: ReactNode;
  className?: string;
}) {
  return (
    <div className={clsx("mx-auto flex max-w-sm flex-col items-center gap-3.5 py-10 text-center", className)}>
      <div className={clsx("grid size-[88px] place-items-center rounded-full", tones[tone].split(" ")[0])}>
        <div className={clsx("grid size-14 place-items-center rounded-2xl bg-canvas shadow-md [&_svg]:size-[26px]", tones[tone].split(" ")[1])}>{icon}</div>
      </div>
      <h3 className="text-base font-semibold tracking-tight">{title}</h3>
      <p className="text-[13px] leading-[18px] text-ink-2">{body}</p>
      {actions && <div className="flex gap-2 pt-1">{actions}</div>}
    </div>
  );
}
