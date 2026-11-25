import { ChevronRight, Database, Info, Keyboard, Settings2, ShieldCheck, Sparkles } from "lucide-react";
import clsx from "clsx";
import type { ReactNode } from "react";
import logo from "../assets/logo.png";
import { TopBar } from "../components/layout/TopBar";
import { AiSettingsPanel } from "./AiSettings";
import { DataStoragePanel } from "./DataStorage";
import { SecuritySettingsPanel } from "./SecuritySettings";
import { Kbd, Toggle } from "../components/ui/Field";
import { api, errorMessage } from "../lib/api";
import { toast } from "../store/toast";
import type { AppBehaviour, Sort, ViewMode } from "../lib/types";
import { ACCENTS, type Accent } from "../lib/accent";
import { formatBytes } from "../lib/format";
import { useCategories } from "../hooks/queries";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { usePrefs, useUi, type SettingsSection } from "../store/ui";

type Section = SettingsSection;

const NAV: { id: Section; label: string; icon: ReactNode; soon?: boolean }[] = [
  { id: "general", label: "General", icon: <Settings2 /> },
  { id: "ai", label: "AI & Categorisation", icon: <Sparkles /> },
  { id: "security", label: "Security", icon: <ShieldCheck /> },
  { id: "data", label: "Data & Storage", icon: <Database /> },
  { id: "shortcuts", label: "Keyboard shortcuts", icon: <Keyboard /> },
  { id: "about", label: "About", icon: <Info /> },
];

function Card({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="flex flex-col gap-2.5 rounded-[14px] border border-line bg-canvas px-5 py-4">
      <h2 className="text-base font-semibold">{title}</h2>
      <div className="flex flex-col divide-y divide-line">{children}</div>
    </section>
  );
}

function Row({ label, hint, children }: { label: string; hint?: string; children: ReactNode }) {
  return (
    <div className="flex items-center gap-3 py-2.5">
      <div className="min-w-0 flex-1">
        <p className="text-[13px] font-medium">{label}</p>
        {hint && <p className="text-xs text-ink-3">{hint}</p>}
      </div>
      {children}
    </div>
  );
}

function Segmented<T extends string>({ value, options, onChange }: { value: T; options: { v: T; label: string }[]; onChange: (v: T) => void }) {
  return (
    <div className="flex gap-0.5 rounded-[9px] bg-muted p-0.5" role="radiogroup">
      {options.map((o) => (
        <button
          key={o.v}
          role="radio"
          aria-checked={value === o.v}
          onClick={() => onChange(o.v)}
          className={clsx("rounded-[7px] px-3 py-1.5 text-[13px] font-medium", value === o.v ? "bg-canvas shadow-sm" : "text-ink-2")}
        >
          {o.label}
        </button>
      ))}
    </div>
  );
}

const selectCls = "h-9 w-[200px] rounded-[9px] border border-line-strong bg-canvas px-2.5 text-[13px]";

function General() {
  const p = usePrefs();
  const qc = useQueryClient();
  const { data: categories = [] } = useCategories();
  const { data: general } = useQuery({ queryKey: ["general-settings"], queryFn: api.getGeneralSettings });
  const { data: storage } = useQuery({ queryKey: ["storage"], queryFn: api.getStorageInfo });
  const b = general?.behaviour;

  const saveBehaviour = async (patch: Partial<AppBehaviour>) => {
    if (!b) return;
    try {
      await api.saveAppBehaviour({ ...b, ...patch });
      await qc.invalidateQueries({ queryKey: ["general-settings"] });
      await qc.invalidateQueries({ queryKey: ["storage"] });
    } catch (e) {
      toast({ type: "error", title: "Couldn't save the setting", body: errorMessage(e) });
    }
  };
  const setAutostart = async (on: boolean) => {
    try {
      await api.setAutostart(on);
      await qc.invalidateQueries({ queryKey: ["general-settings"] });
    } catch (e) {
      toast({ type: "error", title: "Couldn't change the startup setting", body: errorMessage(e) });
    }
  };

  return (
    <>
      <p className="-mt-3 text-[13px] text-ink-2">Appearance, defaults and how Magpie behaves on Windows.</p>
      <Card title="Appearance">
        <Row label="Accent colour" hint="Used for buttons, selection and highlights">
          <div className="flex gap-2" role="radiogroup" aria-label="Accent colour">
            {(Object.keys(ACCENTS) as Accent[]).map((a) => (
              <button
                key={a}
                role="radio"
                aria-checked={p.accent === a}
                aria-label={ACCENTS[a].label}
                title={ACCENTS[a].label}
                onClick={() => p.setAccent(a)}
                className={clsx("size-6 rounded-full ring-offset-2 ring-offset-canvas", p.accent === a && "ring-2")}
                style={{ background: ACCENTS[a].swatch, ["--tw-ring-color" as string]: ACCENTS[a].swatch }}
              />
            ))}
          </div>
        </Row>
      </Card>
      <Card title="Library defaults">
        <Row label="Default view" hint="How bookmarks are shown in every section">
          <Segmented<ViewMode>
            value={p.viewMode}
            onChange={p.setViewMode}
            options={[
              { v: "card", label: "Card" },
              { v: "compact", label: "Compact" },
              { v: "list", label: "List" },
            ]}
          />
        </Row>
        <Row label="Default sort">
          <select value={p.defaultSort} onChange={(e) => p.setDefaultSort(e.target.value as Sort)} className={selectCls}>
            <option value="recently_added">Recently added</option>
            <option value="recently_modified">Recently modified</option>
            <option value="title_asc">Title A–Z</option>
            <option value="domain">Domain</option>
            <option value="most_opened">Most opened</option>
          </select>
        </Row>
        <Row label="Default category for new bookmarks" hint="Used when AI is off or unsure">
          <select
            value={p.defaultCategoryId && categories.some((c) => c.id === p.defaultCategoryId) ? p.defaultCategoryId : ""}
            onChange={(e) => p.setDefaultCategoryId(e.target.value || null)}
            className={selectCls}
          >
            <option value="">Uncategorized</option>
            {categories.map((c) => {
              const parent = c.parentId ? categories.find((x) => x.id === c.parentId) : null;
              return (
                <option key={c.id} value={c.id}>
                  {parent ? `${parent.name} › ${c.name}` : c.name}
                </option>
              );
            })}
          </select>
        </Row>
        <Row label="Confirm before deleting" hint="Ask before moving several bookmarks to Trash">
          <Toggle checked={p.confirmDelete} onChange={p.setConfirmDelete} label="Confirm before deleting" />
        </Row>
      </Card>
      <Card title="Windows">
        <Row label="Launch Magpie when Windows starts" hint={b?.minimiseToTray ? "Starts quietly in the system tray" : undefined}>
          <Toggle checked={!!general?.autostart} onChange={(v) => void setAutostart(v)} label="Launch Magpie when Windows starts" />
        </Row>
        <Row label="Minimise to system tray" hint="Closing the window keeps Magpie running for quick add (Ctrl Alt B)">
          <Toggle checked={!!b?.minimiseToTray} onChange={(v) => void saveBehaviour({ minimiseToTray: v })} label="Minimise to system tray" />
        </Row>
        <Row label="Refresh bookmark previews" hint="Re-fetch titles, descriptions and thumbnails">
          <select value={b?.refreshDays ?? 30} onChange={(e) => void saveBehaviour({ refreshDays: Number(e.target.value) })} className={selectCls}>
            <option value={0}>Never</option>
            <option value={7}>Every 7 days</option>
            <option value={30}>Every 30 days</option>
            <option value={90}>Every 90 days</option>
          </select>
        </Row>
        <Row label="Thumbnail cache limit" hint={storage ? `Currently using ${formatBytes(storage.previewsBytes)}` : undefined}>
          <select value={b?.cacheLimitMb ?? 500} onChange={(e) => void saveBehaviour({ cacheLimitMb: Number(e.target.value) })} className={selectCls}>
            <option value={100}>100 MB</option>
            <option value={250}>250 MB</option>
            <option value={500}>500 MB</option>
            <option value={1000}>1 GB</option>
            <option value={2000}>2 GB</option>
            <option value={0}>No limit</option>
          </select>
        </Row>
      </Card>
      <Card title="Link previews">
        <Row label="Fetch link previews automatically" hint="Downloads each page's title, description, icon and preview image. Local and private network addresses are never fetched.">
          <Toggle checked={p.autoPreviews} onChange={p.setAutoPreviews} label="Fetch link previews automatically" />
        </Row>
        <Row label="Fetch missing previews now" hint="Runs in the background for bookmarks without a preview">
          <button
            className="h-9 rounded-[9px] border border-line-strong px-3 text-[13px] font-medium hover:bg-subtle"
            onClick={() =>
              void api
                .refreshMetadata(null)
                .then(() => toast({ type: "info", title: "Fetching missing previews in the background" }))
                .catch((e) => toast({ type: "error", title: "Couldn't start", body: errorMessage(e) }))
            }
          >
            Fetch now
          </button>
        </Row>
      </Card>
    </>
  );
}

const SHORTCUTS: [string, string[]][] = [
  ["Add bookmark", ["Ctrl", "N"]],
  ["Search", ["Ctrl", "K"]],
  ["Focus search", ["Ctrl", "F"]],
  ["Open favourites", ["Ctrl", "Shift", "F"]],
  ["Open in private window", ["Ctrl", "Shift", "N"]],
  ["Copy URL", ["Ctrl", "Shift", "C"]],
  ["Quick add from anywhere (tray mode)", ["Ctrl", "Alt", "B"]],
  ["Open selected bookmark", ["Ctrl", "Enter"]],
  ["Toggle favourite (selected)", ["Ctrl", "D"]],
  ["Edit selected bookmark", ["F2"]],
  ["Delete selected", ["Delete"]],
  ["Select all visible", ["Ctrl", "A"]],
  ["Close dialog / clear selection", ["Esc"]],
];

function Shortcuts() {
  return (
    <Card title="Keyboard shortcuts">
      {SHORTCUTS.map(([label, keys]) => (
        <Row key={label} label={label}>
          <span className="flex items-center gap-1">
            {keys.map((k, i) => (
              <span key={k} className="flex items-center gap-1">
                {i > 0 && <span className="text-xs text-ink-3">+</span>}
                <Kbd>{k}</Kbd>
              </span>
            ))}
          </span>
        </Row>
      ))}
    </Card>
  );
}

function About() {
  return (
    <>
      <section className="flex items-center gap-4 rounded-[14px] border border-line bg-canvas px-5 py-4">
        <img src={logo} alt="" className="size-16 rounded-2xl" />
        <div className="flex-1">
          <p className="text-xl font-semibold tracking-tight">Magpie</p>
          <p className="text-[13px] text-ink-2">Version 0.1.0 · Milestone 1 preview</p>
          <p className="text-xs text-ink-3">Your bookmarks, organised and safe — stored only on this PC.</p>
        </div>
      </section>
      <section className="flex items-center gap-3 rounded-[14px] border border-brand-100 bg-brand-50 px-4 py-3.5">
        <ShieldCheck className="size-5 text-brand-600" />
        <div>
          <p className="text-[13px] font-medium">Local-first by design</p>
          <p className="text-xs text-ink-2">No account, no cloud sync, no analytics.</p>
        </div>
      </section>
    </>
  );
}

export function Settings({ section }: { section: SettingsSection }) {
  const navigate = useUi((s) => s.navigate);
  const current = NAV.find((n) => n.id === section)!;
  return (
    <div className="flex h-full min-w-0 flex-1 flex-col">
      <TopBar
        crumbs={
          <>
            <Settings2 />
            <span className="text-ink-3">Settings</span>
            <ChevronRight className="!size-3.5" />
            <span className="font-medium">{current.label}</span>
          </>
        }
      />
      <div className="flex min-h-0 flex-1 gap-10 overflow-y-auto px-8 pt-8 pb-10">
        <nav className="flex w-[210px] shrink-0 flex-col gap-0.5">
          <span className="px-2.5 pb-2 text-[11px] font-semibold tracking-[0.04em] text-ink-3">SETTINGS</span>
          {NAV.map((n) => (
            <button
              key={n.id}
              disabled={n.soon}
              onClick={() => !n.soon && navigate({ page: "settings", section: n.id as SettingsSection })}
              className={clsx(
                "flex h-9 items-center gap-2.5 rounded-lg px-2.5 text-sm font-medium [&>svg]:size-[18px]",
                section === n.id ? "bg-brand-50 text-brand-700" : "text-ink-2 enabled:hover:bg-muted",
                n.soon && "cursor-not-allowed opacity-60",
              )}
            >
              {n.icon}
              <span className="flex-1 text-left">{n.label}</span>
              {n.soon && <span className="rounded-md bg-muted px-1.5 text-[11px] text-ink-3">Soon</span>}
            </button>
          ))}
        </nav>
        <div className="flex max-w-[860px] min-w-0 flex-1 flex-col gap-4">
          <div>
            <h1 className="text-xl font-semibold tracking-tight">{current.label}</h1>
          </div>
          {section === "general" && <General />}
          {section === "ai" && <AiSettingsPanel />}
          {section === "security" && <SecuritySettingsPanel />}
          {section === "data" && <DataStoragePanel />}
          {section === "shortcuts" && <Shortcuts />}
          {section === "about" && <About />}
        </div>
      </div>
    </div>
  );
}
