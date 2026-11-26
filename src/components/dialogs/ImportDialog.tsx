import { useQuery, useQueryClient } from "@tanstack/react-query";
import { open as openFile } from "@tauri-apps/plugin-dialog";
import { Check, ChevronDown, Download, FileText, Folder, Globe, Loader2, ShieldCheck } from "lucide-react";
import clsx from "clsx";
import { useEffect, useMemo, useState, type ReactNode } from "react";
import { keys, useCategories } from "../../hooks/queries";
import { api, errorMessage } from "../../lib/api";
import { plural } from "../../lib/format";
import type { BrowserName, ImportFolder, ImportPreview, ImportSource, ImportSummary } from "../../lib/types";
import { usePrefs, useUi } from "../../store/ui";
import { Button } from "../ui/Button";
import { Dialog } from "../ui/Dialog";
import { Toggle } from "../ui/Field";
import { CategorySelect } from "./CategorySelect";

const SEP = "\u001f";
type Kind = BrowserName | "html";
type Step = "source" | "preview" | "done";

const KINDS: { kind: Kind; title: string; icon: ReactNode }[] = [
  { kind: "chrome", title: "Google Chrome", icon: <Globe /> },
  { kind: "edge", title: "Microsoft Edge", icon: <Globe /> },
  { kind: "html", title: "HTML file", icon: <FileText /> },
];

function Steps({ step }: { step: Step }) {
  const i = step === "source" ? 0 : step === "preview" ? 1 : 2;
  return (
    <div className="flex items-center gap-1.5" aria-label={`Step ${i + 1} of 3`}>
      {[0, 1, 2].map((n) => (
        <span key={n} className={clsx("h-1 rounded-full", n === i ? "w-6 bg-brand-600" : n < i ? "w-3 bg-brand-600" : "w-3 bg-line-strong")} />
      ))}
    </div>
  );
}

function Checkbox({ state, onClick, label }: { state: "on" | "off" | "partial"; onClick: () => void; label: string }) {
  return (
    <button
      type="button"
      role="checkbox"
      aria-checked={state === "partial" ? "mixed" : state === "on"}
      aria-label={label}
      onClick={onClick}
      className={clsx("grid size-4 shrink-0 place-items-center rounded border", state === "off" ? "border-line-strong bg-canvas" : "border-brand-600 bg-brand-600 text-white")}
    >
      {state === "on" && <Check className="size-2.5" strokeWidth={3.5} />}
      {state === "partial" && <span className="h-0.5 w-2 rounded bg-white" />}
    </button>
  );
}

function FolderTree({ folders, selected, setSelected }: { folders: ImportFolder[]; selected: Set<string>; setSelected: (s: Set<string>) => void }) {
  const subtree = (key: string) => folders.filter((f) => f.key === key || (key !== "" && f.key.startsWith(key + SEP)));
  const stateOf = (key: string): "on" | "off" | "partial" => {
    const keys = subtree(key).map((f) => f.key);
    const n = keys.filter((k) => selected.has(k)).length;
    return n === 0 ? "off" : n === keys.length ? "on" : "partial";
  };
  const toggle = (key: string) => {
    const next = new Set(selected);
    const on = stateOf(key) !== "on";
    for (const f of subtree(key)) on ? next.add(f.key) : next.delete(f.key);
    setSelected(next);
  };
  const total = (key: string) => subtree(key).reduce((n, f) => n + f.count, 0);
  const allOn = folders.every((f) => selected.has(f.key));

  return (
    <div className="overflow-hidden rounded-xl border border-line">
      <div className="flex items-center border-b border-line bg-subtle px-3.5 py-2">
        <span className="flex-1 text-[11px] font-semibold tracking-[0.04em] text-ink-3">SELECT FOLDERS TO IMPORT</span>
        <button className="text-xs font-medium text-brand-600" onClick={() => setSelected(allOn ? new Set() : new Set(folders.map((f) => f.key)))}>
          {allOn ? "Select none" : "Select all"}
        </button>
      </div>
      <div className="max-h-[232px] overflow-y-auto">
        {folders.map((f) => {
          const st = stateOf(f.key);
          return (
            <div key={f.key || "(root)"} className="flex items-center gap-2.5 border-b border-line py-2 pr-3.5 last:border-b-0" style={{ paddingLeft: 14 + f.depth * 22 }}>
              <Checkbox state={st} onClick={() => toggle(f.key)} label={f.name} />
              <Folder className={clsx("size-4 shrink-0", f.depth === 0 ? "text-ink-2" : "text-brand-500")} />
              <span className={clsx("flex-1 truncate text-[13px]", f.depth === 0 && "font-medium", st === "off" && "text-ink-3")}>{f.name}</span>
              <span className="text-xs font-medium text-ink-3 tabular-nums">{total(f.key)}</span>
            </div>
          );
        })}
      </div>
    </div>
  );
}

function Stat({ label, value, dot }: { label: string; value: number; dot?: string }) {
  return (
    <div className="flex flex-col gap-0.5 rounded-xl border border-line bg-subtle px-3.5 py-2.5">
      <span className="flex items-center gap-1.5 text-xs font-medium text-ink-2">
        {dot && <span className="size-1.5 rounded-full" style={{ background: dot }} />}
        {label}
      </span>
      <span className="text-xl font-semibold tabular-nums">{value.toLocaleString()}</span>
    </div>
  );
}

export function ImportDialog({ initial, onClose }: { initial?: Kind; onClose: () => void }) {
  const qc = useQueryClient();
  const navigate = useUi((s) => s.navigate);
  const setWelcomeDone = usePrefs((s) => s.setWelcomeDone);
  const { data: categories = [] } = useCategories();
  const { data: profiles = [], isLoading: detecting } = useQuery({ queryKey: ["browsers"], queryFn: api.detectBrowsers });

  const [step, setStep] = useState<Step>("source");
  const [kind, setKind] = useState<Kind>(initial ?? "chrome");
  const [profile, setProfile] = useState<string>("");
  const [htmlPath, setHtmlPath] = useState<string | null>(null);
  const [preview, setPreview] = useState<ImportPreview | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [asCategories, setAsCategories] = useState(true);
  const [skipDupes, setSkipDupes] = useState(true);
  const [destination, setDestination] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [summary, setSummary] = useState<ImportSummary | null>(null);

  const forKind = useMemo(() => profiles.filter((p) => p.browser === kind), [profiles, kind]);
  useEffect(() => {
    if (kind !== "html") setProfile((cur) => (forKind.some((p) => p.dir === cur) ? cur : forKind[0]?.dir ?? ""));
  }, [kind, forKind]);

  const source: ImportSource | null = kind === "html" ? (htmlPath ? { kind: "html", path: htmlPath } : null) : profile ? { kind: "browser", browser: kind, profile } : null;
  const selectedCount = preview?.folderTree.filter((f) => selected.has(f.key)).reduce((n, f) => n + f.count, 0) ?? 0;

  const chooseFile = async () => {
    const picked = await openFile({ multiple: false, directory: false, filters: [{ name: "Bookmarks HTML", extensions: ["html", "htm"] }] });
    if (typeof picked === "string") setHtmlPath(picked);
  };

  const loadPreview = async () => {
    if (!source) return;
    setBusy(true);
    setError(null);
    try {
      const p = await api.previewImport(source);
      setPreview(p);
      setSelected(new Set(p.folderTree.map((f) => f.key)));
      setStep("preview");
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  const runImport = async () => {
    if (!source || !preview) return;
    setBusy(true);
    setError(null);
    try {
      const s = await api.startImport(source, {
        selectedFolders: [...selected],
        foldersAsCategories: asCategories,
        skipDuplicates: skipDupes,
        destinationCategoryId: destination,
      });
      setSummary(s);
      setStep("done");
      setWelcomeDone(true);
      if (usePrefs.getState().autoPreviews) void api.refreshMetadata(null).catch(() => undefined);
      await qc.invalidateQueries();
      void qc.invalidateQueries({ queryKey: keys.bookmarks() });
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  const sourceSub = (k: Kind) => {
    if (k === "html") return htmlPath ? htmlPath.split(/[\\/]/).pop()! : "Netscape format from any browser";
    if (detecting) return "Looking for profiles…";
    const ps = profiles.filter((p) => p.browser === k);
    if (!ps.length) return "Not found on this PC";
    return `${plural(ps.length, "profile")} found`;
  };

  const footer =
    step === "done" ? (
      <>
        <div className="flex-1" />
        <Button onClick={onClose}>Close</Button>
        <Button
          variant="primary"
          onClick={() => {
            navigate({ page: "library", view: "all" });
            onClose();
          }}
        >
          View bookmarks
        </Button>
      </>
    ) : (
      <>
        <ShieldCheck className="size-3.5 text-success" />
        <span className="flex-1 text-xs text-ink-2">Read-only — your browser isn't changed.</span>
        {step === "preview" && (
          <Button variant="ghost" onClick={() => setStep("source")} disabled={busy}>
            Back
          </Button>
        )}
        <Button onClick={onClose} disabled={busy}>
          Cancel
        </Button>
        {step === "source" ? (
          <Button variant="primary" onClick={() => void loadPreview()} disabled={!source || busy} icon={busy ? <Loader2 className="animate-spin" /> : undefined}>
            Preview
          </Button>
        ) : (
          <Button variant="primary" onClick={() => void runImport()} disabled={!selectedCount || busy} icon={busy ? <Loader2 className="animate-spin" /> : <Download />}>
            {busy ? "Importing…" : `Import ${plural(selectedCount, "bookmark")}`}
          </Button>
        )}
      </>
    );

  return (
    <Dialog
      open
      onClose={() => !busy && onClose()}
      title="Import bookmarks"
      subtitle={step === "source" ? "Choose where to import from" : step === "preview" ? `Review before importing · ${preview?.sourceLabel}` : "Import complete"}
      icon={<Download />}
      width={720}
      badge={<Steps step={step} />}
      footer={footer}
    >
      <div className="flex flex-col gap-4 px-6 py-5">
        {step === "source" && (
          <>
            <div className="grid grid-cols-3 gap-2.5">
              {KINDS.map((k) => (
                <button
                  key={k.kind}
                  onClick={() => {
                    setKind(k.kind);
                    if (k.kind === "html" && !htmlPath) void chooseFile();
                  }}
                  className={clsx(
                    "flex items-center gap-2.5 rounded-xl border p-3 text-left",
                    kind === k.kind ? "border-[1.5px] border-brand-500 bg-brand-50" : "border-line hover:border-line-strong",
                  )}
                >
                  <span className={clsx("grid size-8 place-items-center rounded-lg border border-line bg-canvas [&_svg]:size-4", kind === k.kind ? "text-brand-600" : "text-ink-2")}>{k.icon}</span>
                  <span className="min-w-0 flex-1">
                    <span className="block text-[13px] font-medium">{k.title}</span>
                    <span className="block truncate text-xs text-ink-3">{sourceSub(k.kind)}</span>
                  </span>
                  {kind === k.kind && (
                    <span className="grid size-[18px] place-items-center rounded-full bg-brand-600 text-white">
                      <Check className="size-3" strokeWidth={3} />
                    </span>
                  )}
                </button>
              ))}
            </div>
            {kind === "html" ? (
              <div className="flex items-center gap-3">
                <Button icon={<FileText />} onClick={() => void chooseFile()}>
                  {htmlPath ? "Choose a different file" : "Choose HTML file…"}
                </Button>
                <span className="truncate text-xs text-ink-3">{htmlPath ?? "Export bookmarks from any browser as HTML, then pick the file."}</span>
              </div>
            ) : forKind.length === 0 ? (
              <p className="text-[13px] text-ink-2">{detecting ? "Looking for browser profiles…" : `${kind === "chrome" ? "Chrome" : "Edge"} isn't installed for this Windows user. Try another browser or an HTML file.`}</p>
            ) : (
              <label className="flex items-center gap-3">
                <span className="text-[13px] text-ink-2">Profile</span>
                <span className="relative">
                  <select
                    value={profile}
                    onChange={(e) => setProfile(e.target.value)}
                    className="h-9 appearance-none rounded-[9px] border border-line-strong bg-canvas pr-8 pl-3 text-[13px] font-medium"
                  >
                    {forKind.map((p) => (
                      <option key={p.dir} value={p.dir}>
                        {p.name}
                      </option>
                    ))}
                  </select>
                  <ChevronDown className="pointer-events-none absolute top-2.5 right-2.5 size-4 text-ink-3" />
                </span>
              </label>
            )}
          </>
        )}

        {step === "preview" && preview && (
          <>
            <div className="grid grid-cols-3 gap-2.5">
              <Stat label="Total detected" value={preview.total} />
              <Stat label="Folders" value={preview.folders} />
              <Stat label="Valid URLs" value={preview.valid} dot="#16A34A" />
              <Stat label="Duplicates in file" value={preview.duplicatesInFile} dot="#D97706" />
              <Stat label="Invalid URLs" value={preview.invalid} dot="#DC2626" />
              <Stat label="Already in library" value={preview.alreadyInLibrary} />
            </div>
            <FolderTree folders={preview.folderTree} selected={selected} setSelected={setSelected} />
            <div className="flex flex-col gap-2.5">
              {[
                { on: asCategories, set: setAsCategories, title: "Import folders as categories", sub: "Keeps your browser's folder structure as nested categories" },
                { on: skipDupes, set: setSkipDupes, title: "Skip duplicates", sub: `${plural(preview.duplicatesInFile + preview.alreadyInLibrary, "bookmark")} match a link you already have` },
              ].map((o) => (
                <div key={o.title} className="flex items-center gap-3">
                  <Toggle checked={o.on} onChange={o.set} label={o.title} />
                  <div>
                    <p className="text-[13px] font-medium">{o.title}</p>
                    <p className="text-xs text-ink-3">{o.sub}</p>
                  </div>
                </div>
              ))}
              <div className="flex items-center gap-3">
                <span className="w-9 shrink-0" />
                <div className="flex flex-1 items-center gap-3">
                  <span className="text-[13px] whitespace-nowrap text-ink-2">Put everything inside</span>
                  <div className="w-64">
                    <CategorySelect categories={categories} value={destination} onChange={setDestination} noneLabel="No parent (top level)" />
                  </div>
                </div>
              </div>
            </div>
            <p className="text-xs text-ink-3">Local URL checks run during import. Titles, descriptions and preview images are then fetched in the background.</p>
          </>
        )}

        {step === "done" && summary && (
          <div className="flex flex-col items-center gap-4 py-4 text-center">
            <span className="grid size-14 place-items-center rounded-full bg-success-bg text-success">
              <Check className="size-7" />
            </span>
            <div>
              <p className="text-lg font-semibold">{plural(summary.imported, "bookmark")} imported</p>
              <p className="text-[13px] text-ink-2">from {preview?.sourceLabel}</p>
            </div>
            <div className="grid w-full grid-cols-4 gap-2.5 text-left">
              <Stat label="Imported" value={summary.imported} dot="#16A34A" />
              <Stat label="Duplicates skipped" value={summary.duplicatesSkipped} dot="#D97706" />
              <Stat label="Invalid skipped" value={summary.invalidSkipped} dot="#DC2626" />
              <Stat label="New categories" value={summary.categoriesCreated} />
            </div>
          </div>
        )}

        {error && <p className="rounded-lg bg-danger-bg px-3 py-2 text-[13px] text-danger">{error}</p>}
      </div>
    </Dialog>
  );
}
