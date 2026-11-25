import { AlertTriangle, Clock, Link2, Loader2, RefreshCw, Shield, ShieldAlert, ShieldCheck, Trash2, X } from "lucide-react";
import clsx from "clsx";
import type { ReactNode } from "react";
import { useFacets, useMarkSafe, useOverrides, useRemoveOverride, useRescan, useSecuritySettings } from "../../hooks/queries";
import { useUi } from "../../store/ui";
import { activeProviders, useExternalScan } from "./ExternalScan";
import { displayUrl, formatDate, formatRelative } from "../../lib/format";
import type { Bookmark } from "../../lib/types";
import { useBookmarkActions } from "../bookmarks/useBookmarkActions";
import { Favicon } from "../bookmarks/visuals";
import { SecurityBadge } from "../ui/Badges";
import { Button } from "../ui/Button";

function Stat({ icon, n, label, box }: { icon: ReactNode; n: number; label: string; box: string }) {
  return (
    <div className="flex flex-1 items-center gap-3 rounded-[14px] border border-line bg-canvas px-4 py-3.5">
      <span className={clsx("grid size-9 place-items-center rounded-[10px] [&_svg]:size-[18px]", box)}>{icon}</span>
      <div>
        <p className="text-xl leading-6 font-semibold tabular-nums">{n.toLocaleString()}</p>
        <p className="text-xs font-medium text-ink-2">{label}</p>
      </div>
    </div>
  );
}

export function SecurityHeaderAction() {
  const rescan = useRescan();
  const scan = useExternalScan();
  const local = (
    <Button
      variant={scan.available ? "secondary" : "primary"}
      icon={<RefreshCw className={clsx(rescan.isPending && "animate-spin")} />}
      disabled={rescan.isPending}
      onClick={() => rescan.mutate(null)}
    >
      {scan.available ? "Re-check locally" : "Scan all bookmarks"}
    </Button>
  );
  if (!scan.available) return local;
  return (
    <>
      {local}
      <Button variant="primary" icon={scan.running ? <Loader2 className="animate-spin" /> : <ShieldCheck />} disabled={scan.running} onClick={() => scan.start(null)}>
        {scan.running && scan.progress ? `Scanning ${scan.progress.done}/${scan.progress.total}…` : scan.running ? "Scanning…" : "Scan with providers"}
      </Button>
      {scan.dialog}
    </>
  );
}

/** Stats and scanner status (Figma 05). */
const SCANNERS = [
  ["google", "Google Safe Browsing"],
  ["urlhaus", "URLhaus"],
  ["virustotal", "VirusTotal"],
] as const;

export function SecuritySummary() {
  const { data: facets } = useFacets();
  const { data: settings } = useSecuritySettings();
  const active = activeProviders(settings);
  const navigate = useUi((s) => s.navigate);
  const n = (s: string) => facets?.security.find((f) => f.value === s)?.count ?? 0;
  return (
    <>
      <div className="flex gap-3">
        <Stat icon={<AlertTriangle />} n={n("dangerous")} label="Dangerous" box="bg-danger-bg text-danger" />
        <Stat icon={<ShieldAlert />} n={n("suspicious")} label="Suspicious" box="bg-warning-bg text-warning" />
        <Stat icon={<Shield />} n={n("not_checked")} label="Local checks only" box="bg-muted text-ink-2" />
        <Stat icon={<ShieldCheck />} n={n("no_threat")} label="No threat detected" box="bg-success-bg text-success" />
      </div>
      <div className="flex flex-wrap items-center gap-x-4 gap-y-2 rounded-xl border border-line bg-subtle px-4 py-3 text-[13px]">
        <span className="font-medium">Scanners</span>
        <span className="flex items-center gap-1.5">
          <span className="size-[7px] rounded-full bg-success" /> Local heuristics
        </span>
        {SCANNERS.map(([id, label]) => {
          const on = settings?.config.enabled && active.includes(id);
          return (
            <span key={id} className={clsx("flex items-center gap-1.5", !on && "text-ink-3")}>
              <span className={clsx("size-[7px] rounded-full", on ? "bg-success" : "bg-ink-3")} /> {label}
              {!on && (
                <button className="rounded-md bg-muted px-1.5 text-xs hover:text-ink" onClick={() => navigate({ page: "settings", section: "security" })}>
                  Set up
                </button>
              )}
            </span>
          );
        })}
        <span className="flex-1" />
        <span className="text-xs text-ink-3">Local checks never mark a site as safe on their own.</span>
      </div>
    </>
  );
}

function AlertCard({ bookmark }: { bookmark: Bookmark }) {
  const a = useBookmarkActions();
  const markSafe = useMarkSafe();
  const rescan = useRescan();
  const danger = bookmark.securityStatus === "dangerous";
  return (
    <article className="flex gap-4 rounded-[14px] border border-line bg-canvas p-4">
      <span className={clsx("w-1 shrink-0 rounded-full", danger ? "bg-danger" : "bg-warning")} aria-hidden />
      <div className="flex min-w-0 flex-1 flex-col gap-2">
        <div className="flex items-center gap-2">
          <Favicon domain={bookmark.domain} file={bookmark.faviconPath} />
          <h3 className="truncate text-sm font-medium">{bookmark.title}</h3>
          <SecurityBadge status={bookmark.securityStatus} />
        </div>
        <p className="flex items-center gap-1.5 truncate text-xs text-ink-2">
          <Link2 className="size-3 shrink-0 text-ink-3" />
          {displayUrl(bookmark.url)}
        </p>
        <ul className="flex flex-col gap-1">
          {bookmark.securityReasons.map((r) => (
            <li key={r} className="flex items-center gap-2 text-[13px] text-ink-2">
              <span className="size-1 rounded-full bg-ink-3" /> {r}
            </li>
          ))}
        </ul>
        <p className="flex items-center gap-1.5 text-xs text-ink-3">
          <Clock className="size-3" /> {bookmark.securitySource ?? "Local heuristics"} · {formatRelative(bookmark.securityCheckedAt)}
        </p>
      </div>
      <div className="flex shrink-0 items-start gap-1.5">
        <Button variant="ghost" size="sm" onClick={() => markSafe.mutate([bookmark.id])} disabled={markSafe.isPending}>
          Mark as safe
        </Button>
        <Button size="sm" icon={<RefreshCw />} onClick={() => rescan.mutate([bookmark.id])} disabled={rescan.isPending}>
          Rescan
        </Button>
        <Button size="sm" icon={<Trash2 />} onClick={() => a.trashNow([bookmark.id])}>
          Move to Trash
        </Button>
      </div>
    </article>
  );
}

export function SecurityAlertList({ items }: { items: Bookmark[] }) {
  return (
    <div className="flex flex-col gap-3">
      {items.map((b) => (
        <AlertCard key={b.id} bookmark={b} />
      ))}
    </div>
  );
}

/** Domains the user marked as safe, with undo. */
export function TrustedSites() {
  const { data: overrides = [] } = useOverrides();
  const remove = useRemoveOverride();
  if (!overrides.length) return null;
  return (
    <section className="flex flex-col gap-2.5 rounded-[14px] border border-line bg-canvas px-4 py-3.5">
      <div>
        <h2 className="text-sm font-semibold">Sites you marked as safe</h2>
        <p className="text-xs text-ink-3">Magpie won't warn about these domains. Remove one to check it again.</p>
      </div>
      <div className="flex flex-wrap gap-1.5">
        {overrides.map((o) => (
          <span key={o.domain} className="inline-flex h-7 items-center gap-1.5 rounded-lg bg-muted pr-1 pl-2.5 text-xs font-medium text-ink-2" title={`Marked safe ${formatDate(o.createdAt)}`}>
            {o.domain}
            <button aria-label={`Stop trusting ${o.domain}`} onClick={() => remove.mutate(o.domain)} className="grid size-5 place-items-center rounded text-ink-3 hover:bg-canvas hover:text-ink">
              <X className="size-3" />
            </button>
          </span>
        ))}
      </div>
    </section>
  );
}
