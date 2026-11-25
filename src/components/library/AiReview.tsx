import { useQueryClient } from "@tanstack/react-query";
import { listen } from "@tauri-apps/api/event";
import { Check, Loader2, Sparkles, X } from "lucide-react";
import { useEffect } from "react";
import { create } from "zustand";
import { useAcceptSuggestions, useAiSettings, useAiSuggestions, useCategories, useCategorize, useRejectSuggestions } from "../../hooks/queries";
import { tint } from "../../lib/colors";
import type { AiProgress, Category, StoredSuggestion } from "../../lib/types";
import { toast } from "../../store/toast";
import { useUi } from "../../store/ui";
import { Favicon } from "../bookmarks/visuals";
import { Button } from "../ui/Button";

export const useAiProgress = create<{ progress: AiProgress | null; set: (p: AiProgress | null) => void }>()((set) => ({
  progress: null,
  set: (progress) => set({ progress }),
}));

/** Listens for background AI runs; mount once at the app root. */
export function useAiProgressListener() {
  const qc = useQueryClient();
  const setProgress = useAiProgress((s) => s.set);
  useEffect(() => {
    const un = listen<AiProgress>("ai-progress", (e) => {
      const p = e.payload;
      setProgress(p.finished ? null : p);
      void qc.invalidateQueries({ queryKey: ["suggestions"] });
      if (p.applied) void qc.invalidateQueries({ queryKey: ["bookmarks"] });
      if (p.finished) {
        void qc.invalidateQueries();
        if (p.error && p.failed && p.done <= p.failed) toast({ type: "error", title: "AI couldn't organise bookmarks", body: p.error });
        else
          toast({
            type: "success",
            title: "AI finished",
            body: [p.suggested && `${p.suggested} suggested`, p.applied && `${p.applied} filed automatically`, p.failed && `${p.failed} failed`].filter(Boolean).join(" · ") || "No confident matches found.",
          });
      }
    }).catch(() => () => undefined);
    return () => void un.then((f) => f());
  }, [qc, setProgress]);
}

function path(cats: Category[], id: string) {
  const c = cats.find((x) => x.id === id);
  if (!c) return null;
  const parent = c.parentId ? cats.find((x) => x.id === c.parentId) : null;
  return { label: parent ? `${parent.name} › ${c.name}` : c.name, color: c.color };
}

function Target({ s, cats }: { s: StoredSuggestion; cats: Category[] }) {
  const t = s.categoryId ? path(cats, s.categoryId) : null;
  if (t)
    return (
      <span className="inline-flex h-6 max-w-[220px] items-center gap-1.5 truncate rounded-full px-2 text-xs font-medium" style={{ background: tint(t.color, 0.12), color: t.color }}>
        <span className="size-1.5 rounded-full" style={{ background: t.color }} /> {t.label}
      </span>
    );
  if (s.newCategory)
    return (
      <span className="inline-flex h-6 items-center gap-1 rounded-full border border-dashed border-brand-300 px-2 text-xs font-medium text-brand-700">
        New · {s.newCategory}
      </span>
    );
  return <span className="text-xs text-ink-3">No good match</span>;
}

/** "AI suggestions" panel for Uncategorized (Figma 11). */
export function AiReviewPanel() {
  const { data: settings } = useAiSettings();
  const enabled = !!settings?.config.enabled;
  const { data: suggestions = [] } = useAiSuggestions(enabled);
  const { data: cats = [] } = useCategories();
  const accept = useAcceptSuggestions();
  const reject = useRejectSuggestions();
  const categorize = useCategorize();
  const progress = useAiProgress((s) => s.progress);
  const navigate = useUi((s) => s.navigate);

  if (!settings) return null;
  if (!enabled)
    return (
      <div className="flex shrink-0 items-center gap-3 rounded-xl border border-brand-200 bg-brand-50 px-4 py-3">
        <Sparkles className="size-[18px] text-brand-600" />
        <p className="flex-1 text-[13px] text-ink-2">Let AI suggest categories for these bookmarks. Bring your own API key — nothing is sent until you turn it on.</p>
        <Button size="sm" onClick={() => navigate({ page: "settings", section: "ai" })}>
          Set up AI
        </Button>
      </div>
    );

  const actionable = suggestions.filter((s) => s.categoryId || s.newCategory);
  const running = !!progress || settings.running;

  return (
    <section className="flex shrink-0 flex-col overflow-hidden rounded-[14px] border border-brand-200 bg-canvas">
      <header className="flex items-center gap-3 bg-brand-50 px-4 py-3">
        <span className="grid size-8 place-items-center rounded-lg bg-brand-600 text-white">
          <Sparkles className="size-4" />
        </span>
        <div className="min-w-0 flex-1">
          <h2 className="text-sm font-semibold">AI suggestions</h2>
          <p className="text-xs text-ink-2">
            {running
              ? progress
                ? `Looking at ${progress.done} of ${progress.total}…`
                : "Working…"
              : actionable.length
                ? `${actionable.length} ready to review · ${settings.config.model}`
                : "Ask AI to suggest a category for each uncategorized bookmark."}
          </p>
        </div>
        {actionable.length > 0 && (
          <>
            <Button size="sm" variant="ghost" onClick={() => reject.mutate(suggestions.map((s) => s.bookmark.id))}>
              Dismiss all
            </Button>
            <Button size="sm" icon={<Check />} onClick={() => accept.mutate(actionable.map((s) => s.bookmark.id))} disabled={accept.isPending}>
              Accept all
            </Button>
          </>
        )}
        <Button size="sm" variant="primary" icon={running ? <Loader2 className="animate-spin" /> : <Sparkles />} disabled={running || categorize.isPending} onClick={() => categorize.mutate(null)}>
          {running ? "Organising…" : "Organise all with AI"}
        </Button>
      </header>
      {progress && (
        <div className="h-1 bg-brand-100">
          <div className="h-full bg-brand-600 transition-[width]" style={{ width: `${Math.round((progress.done / Math.max(progress.total, 1)) * 100)}%` }} />
        </div>
      )}
      {suggestions.length > 0 && (
        <ul className="flex max-h-[360px] flex-col overflow-y-auto">
          {suggestions.map((s) => (
            <li key={s.bookmark.id} className="flex items-center gap-3 border-t border-line px-4 py-2.5">
              <Favicon domain={s.bookmark.domain} file={s.bookmark.faviconPath} size={28} />
              <div className="min-w-0 flex-1">
                <p className="truncate text-[13px] font-medium">{s.bookmark.title}</p>
                <p className="truncate text-xs text-ink-3" title={s.reason ?? undefined}>
                  {s.bookmark.domain}
                  {s.reason ? ` · ${s.reason}` : ""}
                </p>
              </div>
              <Target s={s} cats={cats} />
              <span className="w-10 text-right text-xs text-ink-3 tabular-nums">{Math.round(s.confidence * 100)}%</span>
              <button
                aria-label="Accept suggestion"
                disabled={!s.categoryId && !s.newCategory}
                className="grid size-7 place-items-center rounded-lg text-success hover:bg-success-bg disabled:opacity-30"
                onClick={() => accept.mutate([s.bookmark.id])}
              >
                <Check className="size-4" />
              </button>
              <button aria-label="Dismiss suggestion" className="grid size-7 place-items-center rounded-lg text-ink-3 hover:bg-muted" onClick={() => reject.mutate([s.bookmark.id])}>
                <X className="size-4" />
              </button>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
