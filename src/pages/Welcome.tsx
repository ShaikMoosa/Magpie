import { useQuery } from "@tanstack/react-query";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { ChevronRight, FileText, Globe, Minus, ShieldCheck, Sparkles, Square, Star, X } from "lucide-react";
import clsx from "clsx";
import { useState, type ReactNode } from "react";
import logo from "../assets/logo.png";
import { FolderArt } from "../components/bookmarks/visuals";
import { Button } from "../components/ui/Button";
import { api } from "../lib/api";
import { plural } from "../lib/format";
import type { BrowserName } from "../lib/types";
import { usePrefs, useUi } from "../store/ui";

type Kind = BrowserName | "html";

const win = (() => {
  try {
    return getCurrentWindow();
  } catch {
    return null;
  }
})();

function Float({ icon, text, box, className }: { icon: ReactNode; text: string; box: string; className: string }) {
  return (
    <div className={clsx("absolute flex items-center gap-1.5 rounded-xl bg-canvas py-2 pr-3 pl-2 text-[13px] font-medium shadow-md", className)}>
      <span className={clsx("grid size-6 place-items-center rounded-[7px] [&_svg]:size-3.5", box)}>{icon}</span>
      {text}
    </div>
  );
}

/** First-run screen (Figma 17 — Welcome). */
export function Welcome() {
  const openDialog = useUi((s) => s.openDialog);
  const setWelcomeDone = usePrefs((s) => s.setWelcomeDone);
  const { data: profiles = [] } = useQuery({ queryKey: ["browsers"], queryFn: api.detectBrowsers });
  const best: Kind = profiles.some((p) => p.browser === "chrome") ? "chrome" : profiles.some((p) => p.browser === "edge") ? "edge" : "html";
  const [kind, setKind] = useState<Kind | null>(null);
  const chosen: Kind = kind ?? best;

  const sub = (b: BrowserName) => {
    const ps = profiles.filter((p) => p.browser === b);
    if (!ps.length) return "Not found on this PC";
    return `${plural(ps.length, "profile")} found`;
  };
  const options: { kind: Kind; title: string; sub: string; icon: ReactNode; disabled?: boolean }[] = [
    { kind: "chrome", title: "Import from Google Chrome", sub: sub("chrome"), icon: <Globe />, disabled: !profiles.some((p) => p.browser === "chrome") },
    { kind: "edge", title: "Import from Microsoft Edge", sub: sub("edge"), icon: <Globe />, disabled: !profiles.some((p) => p.browser === "edge") },
    { kind: "html", title: "Import an HTML bookmarks file", sub: "Netscape format from any browser", icon: <FileText /> },
  ];
  const label = chosen === "chrome" ? "Chrome" : chosen === "edge" ? "Edge" : "an HTML file";
  const ctl = "grid h-8 w-10 place-items-center rounded-md text-ink-2 hover:bg-muted [&_svg]:size-3.5";

  return (
    <div className="flex h-full flex-col bg-canvas">
      <header data-tauri-drag-region className="flex h-12 shrink-0 items-center gap-2.5 pr-2 pl-5">
        <img src={logo} alt="" className="size-[22px] rounded-md" draggable={false} />
        <span className="flex-1 text-sm font-medium" data-tauri-drag-region>
          Magpie
        </span>
        <button className={ctl} aria-label="Minimise" onClick={() => win?.minimize()}>
          <Minus strokeWidth={1.6} />
        </button>
        <button className={ctl} aria-label="Maximise" onClick={() => win?.toggleMaximize()}>
          <Square strokeWidth={1.6} className="!size-3" />
        </button>
        <button className={`${ctl} hover:!bg-danger hover:!text-white`} aria-label="Close" onClick={() => win?.close()}>
          <X strokeWidth={1.6} />
        </button>
      </header>
      <div className="flex min-h-0 flex-1 px-4 pb-4">
        <section className="flex flex-1 flex-col justify-center gap-7 overflow-y-auto px-20 py-10">
          <span className="inline-flex items-center gap-2 self-start rounded-full bg-brand-50 px-2.5 py-1 text-xs font-medium text-brand-700">
            <Sparkles className="size-3.5 text-brand-600" /> Step 1 of 2 · Bring your bookmarks
          </span>
          <div className="flex flex-col gap-2.5">
            <h1 className="text-[32px] leading-10 font-bold tracking-[-0.02em]">Welcome to Magpie</h1>
            <p className="max-w-[520px] text-[15px] leading-[22px] text-ink-2">
              One calm, searchable home for every link you save. Start by importing from your browser — nothing in your browser is changed.
            </p>
          </div>
          <div className="flex w-[560px] flex-col gap-2.5" role="radiogroup" aria-label="Import from">
            {options.map((o) => {
              const active = chosen === o.kind;
              return (
                <button
                  key={o.kind}
                  role="radio"
                  aria-checked={active}
                  disabled={o.disabled}
                  onClick={() => setKind(o.kind)}
                  className={clsx(
                    "flex items-center gap-3.5 rounded-[14px] border p-3.5 pl-4 text-left disabled:cursor-not-allowed disabled:opacity-55",
                    active ? "border-[1.5px] border-brand-500 bg-brand-50" : "border-line bg-canvas shadow-sm enabled:hover:border-line-strong",
                  )}
                >
                  <span className={clsx("grid size-10 place-items-center rounded-[10px] border border-line bg-canvas [&_svg]:size-5", active ? "text-brand-600" : "text-ink-2")}>{o.icon}</span>
                  <span className="min-w-0 flex-1">
                    <span className="flex items-center gap-2 text-sm font-medium">
                      {o.title}
                      {o.kind === best && best !== "html" && <span className="rounded-md bg-brand-100 px-1.5 text-xs font-medium text-brand-700">Recommended</span>}
                    </span>
                    <span className="block text-xs text-ink-3">{o.sub}</span>
                  </span>
                  <ChevronRight className={clsx("size-[18px]", active ? "text-brand-600" : "text-ink-3")} />
                </button>
              );
            })}
          </div>
          <div className="flex items-center gap-4">
            <Button variant="primary" icon={<ChevronRight />} onClick={() => openDialog({ kind: "import", source: chosen })}>
              Continue with {label}
            </Button>
            <button className="text-sm font-medium text-ink-2 hover:text-ink" onClick={() => setWelcomeDone(true)}>
              Start with an empty library
            </button>
          </div>
          <p className="flex items-center gap-2 text-xs text-ink-3">
            <ShieldCheck className="size-3.5 text-success" /> Local-first · no account · your library never leaves this PC
          </p>
        </section>
        <aside
          className="relative grid w-[600px] shrink-0 place-items-center overflow-hidden rounded-3xl"
          style={{ background: "linear-gradient(150deg, #F6F0FF 0%, #E9DBFE 55%, #C9A4FF 100%)" }}
          aria-hidden
        >
          <div className="relative h-[560px] w-[520px]">
            <div className="absolute top-[70px] left-10 rotate-6">
              <FolderArt color="#4F8EF7" letters={[{ letter: "R", color: "#20232A" }, { letter: "T", color: "#24C8DB" }, { letter: "G", color: "#181717" }]} count={428} />
            </div>
            <div className="absolute top-10 left-[300px] -rotate-[7deg]">
              <FolderArt color="#EC4899" letters={[{ letter: "S", color: "#FF6719" }, { letter: "M", color: "#111111" }]} count={72} />
            </div>
            <div className="absolute top-[200px] left-[170px]">
              <FolderArt color="#8B5CF6" letters={[{ letter: "F", color: "#1E1E1E" }, { letter: "D", color: "#EA4C89" }, { letter: "M", color: "#3B3F4C" }]} count={312} />
            </div>
            <Float icon={<Star className="fill-warning" />} text="Added to Favourites" box="bg-warning-bg text-warning" className="top-0 left-[300px]" />
            <Float icon={<Sparkles />} text="Suggested: Design › UI Kits · 94%" box="bg-brand-600 text-white" className="top-[440px] left-10" />
            <Float icon={<ShieldCheck />} text="Checked for look-alike sites" box="bg-success-bg text-success" className="top-[500px] left-[270px]" />
          </div>
        </aside>
      </div>
    </div>
  );
}
