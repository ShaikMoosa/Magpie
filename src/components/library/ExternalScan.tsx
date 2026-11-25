import { useQueryClient } from "@tanstack/react-query";
import { listen } from "@tauri-apps/api/event";
import { ShieldCheck } from "lucide-react";
import { useEffect } from "react";
import { create } from "zustand";
import { useSecuritySettings } from "../../hooks/queries";
import { api, errorMessage } from "../../lib/api";
import type { ScanProgress, SecuritySettings } from "../../lib/types";
import { toast } from "../../store/toast";
import { Button } from "../ui/Button";
import { Dialog } from "../ui/Dialog";

const LABELS = { google: "Google Safe Browsing", urlhaus: "URLhaus", virustotal: "VirusTotal" } as const;

/** Pending "send URLs?" confirmation, shown by <ScanConsentHost /> at the app root. */
const useScanAsk = create<{ ask: { ids: string[] | null } | null; set: (a: { ids: string[] | null } | null) => void }>()((set) => ({
  ask: null,
  set: (ask) => set({ ask }),
}));

export const useScanProgress = create<{ progress: ScanProgress | null; set: (p: ScanProgress | null) => void }>()((set) => ({
  progress: null,
  set: (progress) => set({ progress }),
}));

/** Providers that are switched on and have a key. */
export function activeProviders(s: SecuritySettings | undefined) {
  if (!s) return [];
  return (["google", "urlhaus", "virustotal"] as const).filter((p) => s.config[p] && s.keys.includes(p));
}

/** Listens for background scans; mount once at the app root. */
export function useScanProgressListener() {
  const qc = useQueryClient();
  const setProgress = useScanProgress((s) => s.set);
  useEffect(() => {
    const un = listen<ScanProgress>("security-progress", (e) => {
      const p = e.payload;
      setProgress(p.finished ? null : p);
      void qc.invalidateQueries({ queryKey: ["bookmarks"] });
      if (!p.finished) return;
      void qc.invalidateQueries();
      const found = p.dangerous + p.suspicious;
      if (p.error && p.failed >= p.done) toast({ type: "error", title: "Security scan failed", body: p.error });
      else
        toast({
          type: p.dangerous ? "error" : found ? "warning" : "success",
          title: `Scanned ${p.done.toLocaleString()} bookmark${p.done === 1 ? "" : "s"}`,
          body: [p.dangerous && `${p.dangerous} dangerous`, p.suspicious && `${p.suspicious} suspicious`, p.failed && `${p.failed} couldn't be checked`, p.error]
            .filter(Boolean)
            .join(" · ") || "No threats found. This doesn't guarantee a site is safe.",
        });
    }).catch(() => () => undefined);
    return () => void un.then((f) => f());
  }, [qc, setProgress]);
}

async function runScan(ids: string[] | null, confirmed: boolean, expiryDays: number) {
  try {
    const n = await api.scanBookmarks(ids, confirmed);
    if (!n) toast({ type: "info", title: ids ? "Nothing to scan" : "Everything was checked recently", body: ids ? "Sites you marked as safe are never sent." : `Results are kept for ${expiryDays} days.` });
  } catch (e) {
    toast({ type: "error", title: "Couldn't start the scan", body: errorMessage(e) });
  }
}

/** Start an external scan; with consent "ask each time" the user confirms first. */
export function useExternalScan() {
  const { data: settings } = useSecuritySettings();
  const setAsk = useScanAsk((s) => s.set);
  const progress = useScanProgress((s) => s.progress);
  const providers = activeProviders(settings);
  const available = !!settings?.config.enabled && settings.config.consent !== "never" && providers.length > 0;
  const running = !!progress || !!settings?.running;
  const start = (ids: string[] | null) => {
    if (settings?.config.consent === "ask") setAsk({ ids });
    else void runScan(ids, false, settings?.config.expiryDays ?? 7);
  };
  return { start, available, running, progress, providers };
}

/** The consent dialog for scans; render once at the app root. */
export function ScanConsentHost() {
  const { data: settings } = useSecuritySettings();
  const ask = useScanAsk((s) => s.ask);
  const setAsk = useScanAsk((s) => s.set);
  const providers = activeProviders(settings);
  if (!ask || !settings) return null;
  return (
    <Dialog
      open
      onClose={() => setAsk(null)}
      title="Send URLs to security scanners?"
      icon={<ShieldCheck />}
      width={480}
      footer={
        <>
          <div className="flex-1" />
          <Button onClick={() => setAsk(null)}>Cancel</Button>
          <Button
            variant="primary"
            autoFocus
            onClick={() => {
              const ids = ask.ids;
              setAsk(null);
              void runScan(ids, true, settings.config.expiryDays);
            }}
          >
            Send and scan
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-2.5 px-6 py-5 text-sm text-ink-2">
        <p>
          {ask.ids ? (ask.ids.length === 1 ? "This bookmark's address" : `The addresses of ${ask.ids.length} bookmarks`) : `Addresses of bookmarks not checked in the last ${settings.config.expiryDays} days`} will be
          sent to <span className="font-medium text-ink">{providers.map((p) => LABELS[p]).join(", ")}</span>.
        </p>
        <p className="text-xs text-ink-3">
          Login details and #fragments are always removed{settings.config.stripTracking ? ", along with tracking and token-like parameters" : ""}. Sites you marked as safe are never sent.
        </p>
      </div>
    </Dialog>
  );
}
