import { AlertTriangle, Check, Sparkles, X } from "lucide-react";
import clsx from "clsx";
import { useToasts, type ToastType } from "../../store/toast";

const styles: Record<ToastType, { badge: string; Icon: typeof Check }> = {
  success: { badge: "bg-success-bg text-success", Icon: Check },
  info: { badge: "bg-brand-50 text-brand-600", Icon: Sparkles },
  warning: { badge: "bg-warning-bg text-warning", Icon: AlertTriangle },
  error: { badge: "bg-danger-bg text-danger", Icon: X },
};

export function Toaster() {
  const { toasts, dismiss } = useToasts();
  return (
    <div className="pointer-events-none fixed top-16 right-6 z-[60] flex w-[380px] flex-col gap-2.5" role="status" aria-live="polite">
      {toasts.map((t) => {
        const s = styles[t.type];
        return (
          <div key={t.id} className="pointer-events-auto flex items-start gap-3 rounded-xl border border-line bg-canvas px-3.5 py-3 shadow-lg">
            <span className={clsx("grid size-7 shrink-0 place-items-center rounded-full", s.badge)}>
              <s.Icon className="size-3.5" />
            </span>
            <div className="min-w-0 flex-1">
              <p className="text-[13px] font-medium">{t.title}</p>
              {t.body && <p className="text-xs text-ink-2">{t.body}</p>}
            </div>
            {t.action && (
              <button
                className="text-[13px] font-medium text-brand-600 hover:text-brand-700"
                onClick={() => {
                  t.action!.run();
                  dismiss(t.id);
                }}
              >
                {t.action.label}
              </button>
            )}
            <button aria-label="Dismiss" className="text-ink-3 hover:text-ink" onClick={() => dismiss(t.id)}>
              <X className="size-3.5" />
            </button>
          </div>
        );
      })}
    </div>
  );
}
