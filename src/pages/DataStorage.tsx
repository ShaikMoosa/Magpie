import { useQuery, useQueryClient } from "@tanstack/react-query";
import { open as openFile, save as saveFile } from "@tauri-apps/plugin-dialog";
import { Clock, Copy, Download, FileText, FolderOpen, Loader2, RotateCcw, Trash2 } from "lucide-react";
import clsx from "clsx";
import { useState, type ReactNode } from "react";
import { Confirm } from "../components/dialogs/SmallDialogs";
import { Button } from "../components/ui/Button";
import { Toggle } from "../components/ui/Field";
import { api, errorMessage } from "../lib/api";
import { formatBytes, plural } from "../lib/format";
import type { BackupInfo, BackupKind, ExportFormat } from "../lib/types";
import { toast } from "../store/toast";
import { usePrefs, useUi } from "../store/ui";

const KIND: Record<BackupKind, string> = {
  auto: "Automatic",
  manual: "Manual",
  "before-import": "Before import",
  "before-restore": "Before restore",
};

const FORMATS: { id: ExportFormat; title: string; sub: string; ext: string; filter: string }[] = [
  { id: "json", title: "JSON", sub: "Everything: hierarchy, colours, tags, favourites", ext: "json", filter: "JSON" },
  { id: "html", title: "HTML", sub: "Netscape format — re-import into any browser", ext: "html", filter: "Bookmarks HTML" },
  { id: "csv", title: "CSV", sub: "Spreadsheet-friendly, one row per bookmark", ext: "csv", filter: "CSV" },
];

function Card({ title, sub, right, children, className }: { title: string; sub?: string; right?: ReactNode; children: ReactNode; className?: string }) {
  return (
    <section className={clsx("flex flex-col gap-3.5 rounded-[14px] border border-line bg-canvas px-5 py-4", className)}>
      <div className="flex items-start gap-3">
        <div className="flex-1">
          <h2 className="text-base font-semibold">{title}</h2>
          {sub && <p className="text-xs text-ink-3">{sub}</p>}
        </div>
        {right}
      </div>
      {children}
    </section>
  );
}

function when(iso: string) {
  const d = new Date(iso);
  const today = new Date();
  const yesterday = new Date(today.getFullYear(), today.getMonth(), today.getDate() - 1);
  const time = d.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });
  if (d.toDateString() === today.toDateString()) return `Today, ${time}`;
  if (d.toDateString() === yesterday.toDateString()) return `Yesterday, ${time}`;
  return `${d.toLocaleDateString(undefined, { day: "numeric", month: "short", year: "numeric" })}, ${time}`;
}

type Pending = { kind: "restore"; src: { name?: string; path?: string }; label: string; bookmarks: number } | { kind: "trash"; n: number } | { kind: "reset" } | null;

/** Settings › Data & Storage (Figma 14). */
export function DataStoragePanel() {
  const qc = useQueryClient();
  const { data: info } = useQuery({ queryKey: ["storage"], queryFn: api.getStorageInfo });
  const { data: backups = [] } = useQuery({ queryKey: ["backups"], queryFn: api.listBackups });
  const { data: cfg } = useQuery({ queryKey: ["backup-config"], queryFn: api.getBackupConfig });
  const [format, setFormat] = useState<ExportFormat>("json");
  const [busy, setBusy] = useState<string | null>(null);
  const [pending, setPending] = useState<Pending>(null);
  const setWelcomeDone = usePrefs((s) => s.setWelcomeDone);
  const navigate = useUi((s) => s.navigate);

  const refresh = () => qc.invalidateQueries();
  const run = async (key: string, fn: () => Promise<void>) => {
    setBusy(key);
    try {
      await fn();
    } catch (e) {
      toast({ type: "error", title: "Something went wrong", body: errorMessage(e) });
    } finally {
      setBusy(null);
    }
  };

  if (!info || !cfg) return null;

  const total = info.databaseBytes + info.previewsBytes + info.backupsBytes;
  const parts = [
    { label: "Thumbnails", bytes: info.previewsBytes, color: "#3B82F6" },
    { label: "Bookmarks & search index", bytes: info.databaseBytes, color: "#7C3AED" },
    { label: "Backups", bytes: info.backupsBytes, color: "#14B8A6" },
  ];

  const doExport = () =>
    run("export", async () => {
      const f = FORMATS.find((x) => x.id === format)!;
      const stamp = new Date().toISOString().slice(0, 10);
      const path = await saveFile({ defaultPath: `magpie-bookmarks-${stamp}.${f.ext}`, filters: [{ name: f.filter, extensions: [f.ext] }] });
      if (!path) return;
      const n = await api.exportLibrary(format, path);
      toast({ type: "success", title: `Exported ${plural(n, "bookmark")}`, body: path });
    });

  const askRestore = (src: { name?: string; path?: string }, label: string) =>
    run(`restore:${src.name ?? "file"}`, async () => {
      const s = await api.inspectBackup(src);
      setPending({ kind: "restore", src, label, bookmarks: s.bookmarks });
    });

  const restoreFromFile = () =>
    run("restore:file", async () => {
      const path = await openFile({ multiple: false, directory: false, filters: [{ name: "Magpie backup", extensions: ["db"] }] });
      if (typeof path !== "string") return;
      const s = await api.inspectBackup({ path });
      setPending({ kind: "restore", src: { path }, label: path.split(/[\\/]/).pop() ?? path, bookmarks: s.bookmarks });
    });

  const confirm = () => {
    const p = pending;
    if (!p) return;
    if (p.kind === "restore")
      void run("restore", async () => {
        const s = await api.restoreBackup(p.src);
        await qc.resetQueries();
        toast({ type: "success", title: "Library restored", body: `${plural(s.bookmarks, "bookmark")} · ${plural(s.categories, "category", "categories")}. A snapshot of the previous library was saved.` });
      });
    else if (p.kind === "trash")
      void run("trash", async () => {
        const n = await api.emptyTrash();
        await refresh();
        toast({ type: "success", title: `Deleted ${plural(n, "bookmark")} permanently` });
      });
    else
      void run("reset", async () => {
        await api.resetAppData();
        await qc.resetQueries();
        setWelcomeDone(false);
        navigate({ page: "dashboard" });
        toast({ type: "success", title: "Magpie was reset", body: "A backup of your old library is in Settings › Data & Storage." });
      });
  };

  return (
    <>
      <p className="-mt-3 text-[13px] text-ink-2">Everything lives on this PC. Export, back up or move your library whenever you like.</p>

      <Card
        title="Local database"
        sub="SQLite · FTS5 search index"
        right={
          <Button icon={<FolderOpen />} onClick={() => void run("open", api.revealDataFolder)}>
            Open folder
          </Button>
        }
      >
        <div className="flex h-9 items-center gap-2 rounded-[9px] border border-line bg-subtle px-3 text-[13px] text-ink-2">
          <FileText className="size-4 text-ink-3" />
          <span className="min-w-0 flex-1 truncate font-mono text-xs">{info.dbPath}</span>
          <button
            aria-label="Copy path"
            className="grid size-7 place-items-center rounded-md text-ink-3 hover:bg-muted hover:text-ink"
            onClick={() => void navigator.clipboard.writeText(info.dbPath).then(() => toast({ type: "success", title: "Path copied" }))}
          >
            <Copy className="size-4" />
          </button>
        </div>
        <div className="flex items-baseline justify-between">
          <span className="text-sm font-medium">{formatBytes(total)} used</span>
          <span className="text-xs text-ink-3">
            {plural(info.bookmarks, "bookmark")} · {plural(info.categories, "category", "categories")} · {info.trash} in Trash
          </span>
        </div>
        <div className="flex h-2 gap-0.5 overflow-hidden rounded-full bg-muted" role="img" aria-label="Storage breakdown">
          {parts.map((p) => (
            <div key={p.label} style={{ width: `${total ? (p.bytes / total) * 100 : 0}%`, background: p.color }} />
          ))}
        </div>
        <div className="flex flex-wrap gap-x-5 gap-y-1 text-xs text-ink-2">
          {parts.map((p) => (
            <span key={p.label} className="flex items-center gap-1.5">
              <span className="size-2 rounded-full" style={{ background: p.color }} />
              {p.label} <span className="font-medium text-ink">{formatBytes(p.bytes)}</span>
            </span>
          ))}
        </div>
      </Card>

      <div className="grid grid-cols-2 gap-4">
        <Card title="Export" sub="Your data, in open formats">
          <div className="flex flex-col gap-2" role="radiogroup" aria-label="Export format">
            {FORMATS.map((f) => {
              const active = format === f.id;
              return (
                <button
                  key={f.id}
                  role="radio"
                  aria-checked={active}
                  onClick={() => setFormat(f.id)}
                  className={clsx("flex items-center gap-3 rounded-xl border px-3.5 py-2.5 text-left", active ? "border-[1.5px] border-brand-500 bg-brand-50" : "border-line hover:border-line-strong")}
                >
                  <span className={clsx("grid size-4 place-items-center rounded-full border-[1.5px]", active ? "border-brand-600" : "border-line-strong")}>
                    {active && <span className="size-2 rounded-full bg-brand-600" />}
                  </span>
                  <span>
                    <span className="block text-[13px] font-medium">{f.title}</span>
                    <span className="block text-xs text-ink-3">{f.sub}</span>
                  </span>
                </button>
              );
            })}
          </div>
          <Button variant="primary" icon={busy === "export" ? <Loader2 className="animate-spin" /> : <Download />} disabled={!info.bookmarks || !!busy} onClick={() => void doExport()}>
            Export {plural(info.bookmarks, "bookmark")}
          </Button>
        </Card>

        <Card title="Backups" sub="Automatic local snapshots">
          <div className="flex items-center gap-2.5">
            <Toggle
              checked={cfg.daily}
              onChange={(daily) => void run("cfg", async () => void (await api.saveBackupConfig({ ...cfg, daily }), await refresh()))}
              label="Back up daily"
            />
            <span className="flex-1 text-[13px] font-medium">Back up daily</span>
            <label className="flex items-center gap-1.5 text-xs text-ink-2">
              keep last
              <select
                value={cfg.keep}
                onChange={(e) => void run("cfg", async () => void (await api.saveBackupConfig({ ...cfg, keep: Number(e.target.value) }), await refresh()))}
                className="h-7 rounded-md border border-line-strong bg-canvas px-1.5 text-xs"
              >
                {[3, 7, 14, 30].map((n) => (
                  <option key={n} value={n}>
                    {n}
                  </option>
                ))}
              </select>
            </label>
          </div>
          <ul className="flex max-h-[176px] flex-col overflow-y-auto rounded-xl border border-line">
            {backups.length === 0 && <li className="px-3.5 py-4 text-center text-xs text-ink-3">No backups yet.</li>}
            {backups.map((b: BackupInfo) => (
              <li key={b.name} className="flex items-center gap-3 border-b border-line px-3.5 py-2 last:border-b-0">
                <Clock className="size-4 shrink-0 text-ink-3" />
                <div className="min-w-0 flex-1">
                  <p className="text-[13px] font-medium">{when(b.createdAt)}</p>
                  <p className="text-xs text-ink-3">
                    {KIND[b.kind]} · {formatBytes(b.size)}
                  </p>
                </div>
                <button className="text-[13px] font-medium text-brand-600 hover:text-brand-700 disabled:opacity-50" disabled={!!busy} onClick={() => void askRestore({ name: b.name }, when(b.createdAt))}>
                  Restore
                </button>
              </li>
            ))}
          </ul>
          <div className="grid grid-cols-2 gap-2">
            <Button
              icon={busy === "backup" ? <Loader2 className="animate-spin" /> : <Download />}
              disabled={!!busy}
              onClick={() =>
                void run("backup", async () => {
                  const b = await api.createBackup();
                  await refresh();
                  toast({ type: "success", title: "Backup saved", body: `${formatBytes(b.size)} · Open folder shows where backups are kept.` });
                })
              }
            >
              Back up now
            </Button>
            <Button icon={<RotateCcw />} disabled={!!busy} onClick={() => void restoreFromFile()}>
              Restore from file…
            </Button>
          </div>
        </Card>
      </div>

      <section className="flex flex-col rounded-[14px] border border-red-200 bg-canvas px-5 py-4">
        <h2 className="pb-2 text-base font-semibold">Danger zone</h2>
        {[
          {
            label: "Clear thumbnail cache",
            hint: `Frees ${formatBytes(info.previewsBytes)} · previews are re-downloaded when needed`,
            button: (
              <Button
                disabled={!info.previewsBytes || !!busy}
                onClick={() =>
                  void run("cache", async () => {
                    const freed = await api.clearPreviewCache();
                    await refresh();
                    toast({ type: "success", title: `Freed ${formatBytes(freed)}` });
                  })
                }
              >
                Clear cache
              </Button>
            ),
          },
          {
            label: "Permanently delete Trash",
            hint: `${plural(info.trash, "bookmark")} · this cannot be undone`,
            button: (
              <Button className="border-red-200 text-danger" disabled={!info.trash || !!busy} onClick={() => setPending({ kind: "trash", n: info.trash })}>
                Delete {plural(info.trash, "item")}
              </Button>
            ),
          },
          {
            label: "Reset application data",
            hint: "Deletes all bookmarks, categories and settings, and removes API keys from Credential Manager. Backups are kept.",
            button: (
              <Button className="border-red-200 text-danger" disabled={!!busy} onClick={() => setPending({ kind: "reset" })}>
                Reset Magpie…
              </Button>
            ),
          },
        ].map((r, i) => (
          <div key={r.label} className={clsx("flex items-center gap-4 py-3", i > 0 && "border-t border-line")}>
            <div className="min-w-0 flex-1">
              <p className="text-[13px] font-medium">{r.label}</p>
              <p className="text-xs text-ink-3">{r.hint}</p>
            </div>
            {r.button}
          </div>
        ))}
      </section>

      {pending?.kind === "restore" && (
        <Confirm
          title="Restore this backup?"
          icon={<RotateCcw />}
          body={`Your library will be replaced with the backup from ${pending.label} (${plural(pending.bookmarks, "bookmark")}). A snapshot of your current library is saved first, so you can undo this.`}
          confirmLabel="Restore"
          onConfirm={confirm}
          onClose={() => setPending(null)}
        />
      )}
      {pending?.kind === "trash" && (
        <Confirm
          title="Empty Trash?"
          icon={<Trash2 />}
          body={`${plural(pending.n, "bookmark")} will be permanently deleted from this PC. This can't be undone.`}
          confirmLabel={`Delete ${pending.n} permanently`}
          onConfirm={confirm}
          onClose={() => setPending(null)}
        />
      )}
      {pending?.kind === "reset" && (
        <Confirm
          title="Reset Magpie?"
          icon={<Trash2 />}
          body="All bookmarks, categories and settings are deleted and your AI keys are removed from Windows Credential Manager. A backup of the current library is saved first and can be restored from Data & Storage."
          confirmLabel="Reset Magpie"
          onConfirm={confirm}
          onClose={() => setPending(null)}
        />
      )}
    </>
  );
}
