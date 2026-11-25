import { getCurrentWindow } from "@tauri-apps/api/window";
import { Minus, Square, X } from "lucide-react";
import type { ReactNode } from "react";

const win = (() => {
  try {
    return getCurrentWindow();
  } catch {
    return null; // running in a plain browser (e.g. `npm run dev`)
  }
})();

function WindowControls() {
  const btn = "grid h-8 w-10 place-items-center rounded-md text-ink-2 hover:bg-muted [&_svg]:size-3.5";
  return (
    <div className="flex items-center pl-2">
      <button className={btn} aria-label="Minimise" onClick={() => win?.minimize()}>
        <Minus strokeWidth={1.6} />
      </button>
      <button className={btn} aria-label="Maximise" onClick={() => win?.toggleMaximize()}>
        <Square strokeWidth={1.6} className="!size-3" />
      </button>
      <button className={`${btn} hover:!bg-danger hover:!text-white`} aria-label="Close" onClick={() => win?.close()}>
        <X strokeWidth={1.6} />
      </button>
    </div>
  );
}

/** Top bar for the main area. Empty space drags the frameless window. */
export function TopBar({ crumbs, actions }: { crumbs: ReactNode; actions?: ReactNode }) {
  return (
    <header
      data-tauri-drag-region
      onDoubleClick={(e) => {
        if (e.target === e.currentTarget) void win?.toggleMaximize();
      }}
      className="flex h-14 shrink-0 items-center gap-3 border-b border-line pr-2 pl-6"
    >
      <div className="flex min-w-0 flex-1 items-center gap-2 text-sm [&_svg]:size-4 [&_svg]:text-ink-3" data-tauri-drag-region>
        {crumbs}
      </div>
      {actions}
      <WindowControls />
    </header>
  );
}
