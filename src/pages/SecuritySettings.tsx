import { useQueryClient } from "@tanstack/react-query";
import { Check, KeyRound, Loader2, Plus, ShieldCheck, X } from "lucide-react";
import clsx from "clsx";
import { useState, type ReactNode } from "react";
import { activeProviders, useExternalScan } from "../components/library/ExternalScan";
import { Button } from "../components/ui/Button";
import { Toggle } from "../components/ui/Field";
import { keys, useOverrides, useRemoveOverride, useSecuritySettings } from "../hooks/queries";
import { api, errorMessage } from "../lib/api";
import { formatRelative } from "../lib/format";
import type { ScanConsent, ScanProvider, ScanSchedule, SecurityConfig } from "../lib/types";
import { toast } from "../store/toast";

const PROVIDERS: { id: ScanProvider; name: string; sub: string; letter: string; color: string; keyUrl: string }[] = [
  { id: "google", name: "Google Safe Browsing", sub: "Phishing, malware and unwanted software", letter: "G", color: "#4285F4", keyUrl: "Google Cloud console" },
  { id: "urlhaus", name: "URLhaus (abuse.ch)", sub: "Malware URL database · free Auth-Key from abuse.ch", letter: "U", color: "#0F766E", keyUrl: "auth.abuse.ch" },
  { id: "virustotal", name: "VirusTotal", sub: "70+ engines · free tier 4 lookups/min, so used for scans of up to 10 bookmarks", letter: "V", color: "#3B5BFD", keyUrl: "virustotal.com" },
];

function Card({ title, sub, children }: { title: string; sub?: string; children: ReactNode }) {
  return (
    <section className="flex flex-col gap-1 rounded-[14px] border border-line bg-canvas px-5 py-4">
      <h2 className="text-base font-semibold">{title}</h2>
      {sub && <p className="text-xs text-ink-3">{sub}</p>}
      <div className="mt-1 flex flex-col">{children}</div>
    </section>
  );
}

function Row({ label, hint, children, first }: { label: string; hint?: ReactNode; children: ReactNode; first?: boolean }) {
  return (
    <div className={clsx("flex items-center gap-3 py-2.5", !first && "border-t border-line")}>
      <div className="min-w-0 flex-1">
        <p className="text-[13px] font-medium">{label}</p>
        {hint && <p className="text-xs text-ink-3">{hint}</p>}
      </div>
      {children}
    </div>
  );
}

function ProviderRow({ p, on, hasKey, checks, onToggle }: { p: (typeof PROVIDERS)[number]; on: boolean; hasKey: boolean; checks: number; onToggle: (v: boolean) => void }) {
  const qc = useQueryClient();
  const [editing, setEditing] = useState(false);
  const [key, setKey] = useState("");
  const [busy, setBusy] = useState(false);

  const save = async () => {
    setBusy(true);
    try {
      await api.setScanKey(p.id, key.trim());
      setKey("");
      setEditing(false);
      await qc.invalidateQueries({ queryKey: keys.securitySettings });
      if (!on) onToggle(true);
      toast({ type: "success", title: `${p.name} connected`, body: "Key checked and stored in Windows Credential Manager." });
    } catch (e) {
      toast({ type: "error", title: "Couldn't use that key", body: errorMessage(e) });
    } finally {
      setBusy(false);
    }
  };
  const remove = async () => {
    await api.removeScanKey(p.id);
    if (on) onToggle(false);
    await qc.invalidateQueries({ queryKey: keys.securitySettings });
  };

  return (
    <div className="flex flex-col gap-2 border-t border-line py-2.5">
      <div className="flex items-center gap-3">
        <span className="grid size-[30px] shrink-0 place-items-center rounded-lg text-[13px] font-bold text-white" style={{ background: p.color }}>
          {p.letter}
        </span>
        <div className="min-w-0 flex-1">
          <p className="text-[13px] font-medium">{p.name}</p>
          <p className="truncate text-xs text-ink-3">{hasKey ? `API key connected · ${checks.toLocaleString()} checks this month` : p.sub}</p>
        </div>
        {hasKey ? (
          <>
            <button className="text-xs font-medium text-ink-3 hover:text-danger" onClick={() => void remove()}>
              Remove key
            </button>
            {on && (
              <span className="inline-flex items-center gap-1 rounded-full bg-success-bg px-2 py-0.5 text-xs font-medium text-success">
                <Check className="size-3" /> Connected
              </span>
            )}
          </>
        ) : (
          !editing && (
            <Button size="sm" icon={<Plus />} onClick={() => setEditing(true)}>
              Add API key
            </Button>
          )
        )}
        <Toggle checked={on && hasKey} onChange={(v) => (hasKey ? onToggle(v) : setEditing(true))} label={`Use ${p.name}`} />
      </div>
      {editing && (
        <form
          className="ml-[42px] flex items-center gap-2"
          onSubmit={(e) => {
            e.preventDefault();
            if (key.trim()) void save();
          }}
        >
          <div className="flex h-9 flex-1 items-center gap-2 rounded-[9px] border border-line-strong px-3 focus-within:border-brand-500">
            <KeyRound className="size-4 text-ink-3" />
            <input
              type="password"
              autoFocus
              autoComplete="off"
              value={key}
              onChange={(e) => setKey(e.target.value)}
              placeholder={`Paste your ${p.name} key (from ${p.keyUrl})`}
              className="min-w-0 flex-1 bg-transparent text-[13px] outline-none"
              aria-label={`${p.name} API key`}
            />
          </div>
          <Button type="submit" size="sm" variant="primary" disabled={!key.trim() || busy} icon={busy ? <Loader2 className="animate-spin" /> : undefined}>
            {busy ? "Checking…" : "Save key"}
          </Button>
          <button type="button" aria-label="Cancel" className="grid size-8 place-items-center text-ink-3" onClick={() => setEditing(false)}>
            <X className="size-4" />
          </button>
        </form>
      )}
    </div>
  );
}

function Segmented<T extends string>({ value, onChange, options }: { value: T; onChange: (v: T) => void; options: { v: T; label: string }[] }) {
  return (
    <div className="flex rounded-[10px] bg-muted p-0.5" role="radiogroup">
      {options.map((o) => (
        <button
          key={o.v}
          role="radio"
          aria-checked={value === o.v}
          onClick={() => onChange(o.v)}
          className={clsx("h-8 rounded-lg px-3 text-[13px] font-medium", value === o.v ? "bg-canvas shadow-sm" : "text-ink-2 hover:text-ink")}
        >
          {o.label}
        </button>
      ))}
    </div>
  );
}

/** Settings › Security (Figma 13). */
export function SecuritySettingsPanel() {
  const qc = useQueryClient();
  const { data: s } = useSecuritySettings();
  const { data: overrides = [] } = useOverrides();
  const removeOverride = useRemoveOverride();
  const scan = useExternalScan();
  if (!s) return null;
  const cfg = s.config;
  const active = activeProviders(s);

  const patch = async (p: Partial<SecurityConfig>) => {
    try {
      await api.saveSecurityConfig({ ...cfg, ...p });
      await qc.invalidateQueries({ queryKey: keys.securitySettings });
    } catch (e) {
      toast({ type: "error", title: "Couldn't save security settings", body: errorMessage(e) });
    }
  };

  return (
    <>
      <p className="-mt-3 text-[13px] text-ink-2">Control how URLs are checked. A “No threat detected” result never guarantees a site is safe.</p>

      <Card title="URL scanning">
        <Row first label="Enable URL security scanning" hint="Checks links with the providers below. Every new URL always gets local validation.">
          <Toggle checked={cfg.enabled} onChange={(v) => void patch({ enabled: v })} label="Enable URL security scanning" />
        </Row>
        <Row label="Warn before opening suspicious sites">
          <Toggle checked={cfg.warnSuspicious} onChange={(v) => void patch({ warnSuspicious: v })} label="Warn before opening suspicious sites" />
        </Row>
        <Row label="Block known malicious sites" hint="You can still open them from the warning dialog after confirming the risk">
          <Toggle checked={cfg.blockDangerous} onChange={(v) => void patch({ blockDangerous: v })} label="Block known malicious sites" />
        </Row>
        <Row label="Scan automatically" hint={cfg.consent === "always" ? undefined : "Automatic scans need “Send URLs to external scanners” set to Always"}>
          <select
            value={cfg.schedule}
            onChange={(e) => void patch({ schedule: e.target.value as ScanSchedule })}
            className="h-9 w-[200px] rounded-[9px] border border-line-strong bg-canvas px-2.5 text-[13px]"
          >
            <option value="manual">Only when I ask</option>
            <option value="on_save">On save</option>
            <option value="on_save_weekly">On save + weekly</option>
          </select>
        </Row>
        <Row label="Keep results for" hint="Older results are refreshed by the weekly scan">
          <select
            value={cfg.expiryDays}
            onChange={(e) => void patch({ expiryDays: Number(e.target.value) })}
            className="h-9 w-[200px] rounded-[9px] border border-line-strong bg-canvas px-2.5 text-[13px]"
          >
            {[1, 7, 14, 30].map((d) => (
              <option key={d} value={d}>
                {d === 1 ? "1 day" : `${d} days`}
              </option>
            ))}
          </select>
        </Row>
        <Row
          label="Scan library now"
          hint={
            !cfg.enabled
              ? "Turn on URL scanning first"
              : !active.length
                ? "Connect a provider below first"
                : s.usage.lastScanAt
                  ? `Last scan ${formatRelative(s.usage.lastScanAt)}${s.usage.lastError ? ` · last error: ${s.usage.lastError}` : ""}`
                  : "Checks bookmarks without a recent result"
          }
        >
          <Button
            icon={scan.running ? <Loader2 className="animate-spin" /> : <ShieldCheck />}
            disabled={!scan.available || scan.running}
            onClick={() => scan.start(null)}
          >
            {scan.running && scan.progress ? `Scanning ${scan.progress.done}/${scan.progress.total}…` : scan.running ? "Scanning…" : "Scan now"}
          </Button>
        </Row>
      </Card>

      <Card title="Threat intelligence providers" sub="URLs are only sent to providers you enable. Keys are stored in Windows Credential Manager.">
        <div className="flex items-center gap-3 py-2.5">
          <span className="grid size-[30px] place-items-center rounded-lg bg-ink-2 text-[13px] font-bold text-white">L</span>
          <div className="flex-1">
            <p className="text-[13px] font-medium">Local heuristics</p>
            <p className="text-xs text-ink-3">Lookalikes, punycode, IP hosts, program downloads, HTTP</p>
          </div>
          <span className="rounded-md bg-muted px-2 py-0.5 text-xs font-medium text-ink-2">Built-in</span>
        </div>
        {PROVIDERS.map((p) => (
          <ProviderRow key={p.id} p={p} on={cfg[p.id]} hasKey={s.keys.includes(p.id)} checks={s.usage[p.id]} onToggle={(v) => void patch({ [p.id]: v })} />
        ))}
      </Card>

      <Card title="Privacy & overrides">
        <Row
          first
          label="Send URLs to external scanners"
          hint={cfg.consent === "ask" ? "Magpie asks before every scan; nothing is sent automatically" : cfg.consent === "always" ? "Private tokens in URLs can be exposed to providers" : "Only local checks run"}
        >
          <Segmented<ScanConsent>
            value={cfg.consent}
            onChange={(v) => void patch({ consent: v })}
            options={[
              { v: "ask", label: "Ask each time" },
              { v: "always", label: "Always" },
              { v: "never", label: "Never" },
            ]}
          />
        </Row>
        <Row label="Remove tracking and token parameters before sending" hint="Login details and #fragments are always removed">
          <Toggle checked={cfg.stripTracking} onChange={(v) => void patch({ stripTracking: v })} label="Remove tracking and token parameters before sending" />
        </Row>
        <Row label="False-positive overrides" hint="Never warn for these domains, and never send them to scanners">
          <div className="flex max-w-[420px] flex-wrap justify-end gap-1.5">
            {overrides.length === 0 && <span className="text-xs text-ink-3">None yet. Use “Mark as safe” on a warning.</span>}
            {overrides.map((o) => (
              <span key={o.domain} className="inline-flex h-7 items-center gap-1 rounded-lg border border-line bg-subtle pr-1 pl-2.5 text-xs">
                {o.domain}
                <button aria-label={`Stop trusting ${o.domain}`} className="grid size-5 place-items-center rounded text-ink-3 hover:text-danger" onClick={() => removeOverride.mutate(o.domain)}>
                  <X className="size-3" />
                </button>
              </span>
            ))}
          </div>
        </Row>
      </Card>
      {scan.dialog}
    </>
  );
}
