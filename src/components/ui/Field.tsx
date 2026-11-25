import clsx from "clsx";
import { forwardRef, type InputHTMLAttributes, type ReactNode, type TextareaHTMLAttributes } from "react";

export function Field({ label, hint, error, children, className }: { label: string; hint?: ReactNode; error?: string | null; children: ReactNode; className?: string }) {
  return (
    <label className={clsx("flex flex-col gap-1.5", className)}>
      <span className="text-xs font-medium text-ink-2">{label}</span>
      {children}
      {error ? <span className="text-xs text-danger">{error}</span> : hint ? <span className="text-xs text-ink-3">{hint}</span> : null}
    </label>
  );
}

const box =
  "w-full rounded-[10px] border border-line-strong bg-canvas px-3 text-sm text-ink placeholder:text-ink-3 outline-none transition focus:border-brand-500 focus:ring-[3px] focus:ring-brand-500/15";

export const TextInput = forwardRef<HTMLInputElement, InputHTMLAttributes<HTMLInputElement> & { leading?: ReactNode; trailing?: ReactNode; invalid?: boolean }>(
  function TextInput({ leading, trailing, invalid, className, ...rest }, ref) {
    return (
      <div className={clsx(box, "flex h-10 items-center gap-2 focus-within:border-brand-500 focus-within:ring-[3px] focus-within:ring-brand-500/15", invalid && "border-danger", className)}>
        {leading && <span className="text-ink-3 [&_svg]:size-4">{leading}</span>}
        <input ref={ref} className="h-full min-w-0 flex-1 bg-transparent outline-none placeholder:text-ink-3 focus-visible:outline-none" {...rest} />
        {trailing}
      </div>
    );
  },
);

export function TextArea(props: TextareaHTMLAttributes<HTMLTextAreaElement>) {
  return <textarea {...props} className={clsx(box, "min-h-16 resize-none py-2.5 leading-5 focus-visible:outline-none", props.className)} />;
}

export function Toggle({ checked, onChange, label }: { checked: boolean; onChange: (v: boolean) => void; label: string }) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      onClick={() => onChange(!checked)}
      className={clsx("relative inline-flex h-5 w-9 shrink-0 items-center rounded-full p-0.5 transition-colors", checked ? "bg-brand-600" : "bg-line-strong")}
    >
      <span className={clsx("size-4 rounded-full bg-white shadow-sm transition-transform", checked && "translate-x-4")} />
    </button>
  );
}

export function Kbd({ children }: { children: ReactNode }) {
  return <kbd className="rounded-md border border-b-2 border-line-strong bg-subtle px-1.5 py-px font-sans text-[11px] font-medium text-ink-3">{children}</kbd>;
}
