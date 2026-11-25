import { useQueryClient } from "@tanstack/react-query";
import { Check, ChevronDown, KeyRound, Loader2, RefreshCw, ShieldCheck, X } from "lucide-react";
import clsx from "clsx";
import { useEffect, useState, type ReactNode } from "react";
import { Button } from "../components/ui/Button";
import { Toggle } from "../components/ui/Field";
import { keys, useAiSettings, useCategories } from "../hooks/queries";
import { api, errorMessage } from "../lib/api";
import { tint } from "../lib/colors";
import { formatRelative } from "../lib/format";
import type { AiConfig, AiMode, AiProvider } from "../lib/types";
import { toast } from "../store/toast";

const PROVIDERS: { id: AiProvider; name: string; sub: string; letter: string; color: string }[] = [
  { id: "openai", name: "OpenAI", sub: "GPT models", letter: "O", color: "#111111" },
  { id: "anthropic", name: "Anthropic", sub: "Claude models", letter: "A", color: "#D97757" },
  { id: "gemini", name: "Google Gemini", sub: "Gemini models", letter: "G", color: "#4285F4" },
  { id: "custom", name: "Custom", sub: "OpenAI-compatible", letter: "C", color: "#64748B" },
];

const MODES: { id: AiMode; title: string; sub: string }[] = [
  { id: "manual", title: "Manual", sub: "You pick every category" },
  { id: "suggestion", title: "Suggestion", sub: "AI suggests, you confirm" },
  { id: "automatic", title: "Automatic", sub: "AI files into existing categories" },
];

function Card({ title, sub, children, right }: { title: string; sub?: string; children: ReactNode; right?: ReactNode }) {
  return (
    <section className="flex flex-col gap-3.5 rounded-[14px] border border-line bg-canvas px-5 py-4">
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

function Row({ label, hint, children, last }: { label: string; hint?: string; children: ReactNode; last?: boolean }) {
  return (
    <div className={clsx("flex items-center gap-3 py-2.5", !last && "border-b border-line")}>
      <div className="min-w-0 flex-1">
        <p className="text-[13px] font-medium">{label}</p>
        {hint && <p className="text-xs text-ink-3">{hint}</p>}
      </div>
      {children}
    </div>
  );
}

export function AiSettingsPanel() {
  const qc = useQueryClient();
  const { data: settings } = useAiSettings();
  const { data: categories = [] } = useCategories();
  const [cfg, setCfg] = useState<AiConfig | null>(null);
  const [keyInput, setKeyInput] = useState("");
  const [editingKey, setEditingKey] = useState(false);
  const [models, setModels] = useState<string[]>([]);
  const [testing, setTesting] = useState<"idle" | "busy" | "ok" | "fail">("idle");
  const [testMsg, setTestMsg] = useState<string | null>(null);

  useEffect(() => {
    if (settings && !cfg) setCfg(settings.config);
  }, [settings, cfg]);

  if (!settings || !cfg) return null;
  const hasKey = settings.keys.includes(cfg.provider);
  const provider = PROVIDERS.find((p) => p.id === cfg.provider)!;

  const save = async (next: AiConfig) => {
    setCfg(next);
    try {
      const saved = await api.saveAiConfig(next);
      setCfg(saved);
      await qc.invalidateQueries({ queryKey: keys.ai });
    } catch (e) {
      toast({ type: "error", title: "Couldn't save AI settings", body: errorMessage(e) });
    }
  };
  const patch = (p: Partial<AiConfig>) => void save({ ...cfg, ...p });

  const loadModels = async (c = cfg) => {
    setTesting("busy");
    setTestMsg(null);
    try {
      const list = await api.listAiModels(c);
      setModels(list);
      setTesting("ok");
      setTestMsg(`Connected · ${list.length} model${list.length === 1 ? "" : "s"} available`);
    } catch (e) {
      setTesting("fail");
      setTestMsg(errorMessage(e));
    }
  };

  const saveKey = async () => {
    try {
      await api.setAiKey(cfg.provider, keyInput.trim());
      setKeyInput("");
      setEditingKey(false);
      await qc.invalidateQueries({ queryKey: keys.ai });
      toast({ type: "success", title: "API key saved", body: "Stored in Windows Credential Manager." });
      void loadModels();
    } catch (e) {
      toast({ type: "error", title: "Couldn't save the key", body: errorMessage(e) });
    }
  };

  const removeKey = async () => {
    await api.removeAiKey(cfg.provider);
    await qc.invalidateQueries({ queryKey: keys.ai });
    setModels([]);
    setTesting("idle");
    toast({ type: "success", title: "API key removed" });
  };

  const usage = settings.usage;
  const top = categories.filter((c) => !c.parentId);

  return (
    <>
      <Card
        title="AI categorisation"
        sub="Bring your own API key. Only a bookmark's URL, title and description are sent — never your whole library."
        right={<Toggle checked={cfg.enabled} onChange={(v) => patch({ enabled: v })} label="Turn on AI categorisation" />}
      >
        {!cfg.enabled && <p className="text-[13px] text-ink-2">AI is off. Everything else in Magpie works without it.</p>}
      </Card>

      <Card title="Provider" sub="Choose who categorises your bookmarks">
        <div className="grid grid-cols-4 gap-2.5">
          {PROVIDERS.map((p) => {
            const active = cfg.provider === p.id;
            return (
              <button
                key={p.id}
                onClick={() => {
                  setModels([]);
                  setTesting("idle");
                  setEditingKey(false);
                  patch({ provider: p.id, model: p.id === "anthropic" ? "claude-opus-5-5" : "" });
                }}
                className={clsx("flex items-center gap-2.5 rounded-xl border p-3 text-left", active ? "border-[1.5px] border-brand-500 bg-brand-50" : "border-line hover:border-line-strong")}
              >
                <span className="grid size-[30px] place-items-center rounded-lg text-[13px] font-bold text-white" style={{ background: p.color }}>
                  {p.letter}
                </span>
                <span className="min-w-0 flex-1">
                  <span className="block text-[13px] font-medium">{p.name}</span>
                  <span className="block truncate text-xs text-ink-3">{settings.keys.includes(p.id) ? "Key saved" : p.sub}</span>
                </span>
                {active && (
                  <span className="grid size-[18px] place-items-center rounded-full bg-brand-600 text-white">
                    <Check className="size-3" strokeWidth={3} />
                  </span>
                )}
              </button>
            );
          })}
        </div>

        {cfg.provider === "custom" && (
          <label className="flex flex-col gap-1.5">
            <span className="text-xs font-medium text-ink-2">Endpoint (OpenAI-compatible base URL)</span>
            <input
              defaultValue={cfg.endpoint}
              onBlur={(e) => e.target.value !== cfg.endpoint && patch({ endpoint: e.target.value })}
              placeholder="http://localhost:11434/v1"
              className="h-10 rounded-[10px] border border-line-strong px-3 text-sm outline-none focus:border-brand-500"
            />
          </label>
        )}

        <div className="grid grid-cols-[1fr_260px] gap-4">
          <div className="flex flex-col gap-1.5">
            <span className="text-xs font-medium text-ink-2">API key{cfg.provider === "custom" ? " (optional)" : ""}</span>
            {hasKey && !editingKey ? (
              <div className="flex h-10 items-center gap-2 rounded-[10px] border border-line-strong px-3">
                <KeyRound className="size-4 text-ink-3" />
                <span className="flex-1 text-sm tracking-widest text-ink-2">••••••••••••••••••••••</span>
                <button className="text-[13px] font-medium text-brand-600" onClick={() => setEditingKey(true)}>
                  Replace
                </button>
              </div>
            ) : (
              <form
                className="flex h-10 items-center gap-2 rounded-[10px] border border-line-strong pr-1 pl-3 focus-within:border-brand-500"
                onSubmit={(e) => {
                  e.preventDefault();
                  if (keyInput.trim()) void saveKey();
                }}
              >
                <KeyRound className="size-4 text-ink-3" />
                <input
                  type="password"
                  autoComplete="off"
                  value={keyInput}
                  onChange={(e) => setKeyInput(e.target.value)}
                  placeholder={`Paste your ${provider.name} API key`}
                  className="min-w-0 flex-1 bg-transparent text-sm outline-none"
                  aria-label="API key"
                />
                {editingKey && (
                  <button type="button" aria-label="Cancel" className="grid size-8 place-items-center text-ink-3" onClick={() => setEditingKey(false)}>
                    <X className="size-4" />
                  </button>
                )}
                <Button type="submit" size="sm" variant="primary" disabled={!keyInput.trim()}>
                  Save key
                </Button>
              </form>
            )}
            <span className="flex items-center gap-1.5 text-xs text-ink-3">
              <ShieldCheck className="size-3.5 text-success" /> Stored in Windows Credential Manager · never saved to the database
            </span>
          </div>
          <label className="flex flex-col gap-1.5">
            <span className="text-xs font-medium text-ink-2">Model</span>
            <span className="relative">
              {models.length ? (
                <select
                  value={cfg.model}
                  onChange={(e) => patch({ model: e.target.value })}
                  className="h-10 w-full appearance-none rounded-[10px] border border-line-strong bg-canvas pr-8 pl-3 text-sm"
                >
                  {!models.includes(cfg.model) && <option value={cfg.model}>{cfg.model || "Choose a model"}</option>}
                  {models.map((m) => (
                    <option key={m} value={m}>
                      {m}
                    </option>
                  ))}
                </select>
              ) : (
                <input
                  defaultValue={cfg.model}
                  key={cfg.provider}
                  onBlur={(e) => e.target.value !== cfg.model && patch({ model: e.target.value })}
                  placeholder="Test the connection to list models"
                  className="h-10 w-full rounded-[10px] border border-line-strong px-3 text-sm outline-none focus:border-brand-500"
                />
              )}
              {models.length > 0 && <ChevronDown className="pointer-events-none absolute top-3 right-3 size-4 text-ink-3" />}
            </span>
          </label>
        </div>

        <div className="flex items-center gap-3">
          <Button icon={testing === "busy" ? <Loader2 className="animate-spin" /> : <RefreshCw />} disabled={testing === "busy" || (!hasKey && cfg.provider !== "custom")} onClick={() => void loadModels()}>
            Test connection
          </Button>
          {testMsg && (
            <span className={clsx("inline-flex items-center gap-1.5 rounded-full px-2.5 py-1 text-xs font-medium", testing === "ok" ? "bg-success-bg text-success" : "bg-danger-bg text-danger")}>
              {testing === "ok" ? <Check className="size-3" /> : <X className="size-3" />} {testMsg}
            </span>
          )}
          <span className="flex-1" />
          {hasKey && (
            <button className="text-[13px] font-medium text-danger" onClick={() => void removeKey()}>
              Remove key
            </button>
          )}
        </div>
      </Card>

      <Card title="Categorisation" sub="How AI applies categories to new and imported bookmarks">
        <div className="flex gap-1 rounded-xl bg-muted p-1" role="radiogroup">
          {MODES.map((m) => (
            <button
              key={m.id}
              role="radio"
              aria-checked={cfg.mode === m.id}
              onClick={() => patch({ mode: m.id })}
              className={clsx("flex-1 rounded-[9px] px-3.5 py-2.5 text-left", cfg.mode === m.id && "bg-canvas shadow-sm")}
            >
              <span className={clsx("block text-[13px] font-medium", cfg.mode === m.id && "text-brand-700")}>{m.title}</span>
              <span className="block text-xs text-ink-3">{m.sub}</span>
            </button>
          ))}
        </div>
        <div className="flex flex-col">
          <Row label="Allow AI to create new categories" hint={cfg.allowNewCategories ? "AI may suggest a new top-level category when nothing fits" : "Off — AI only suggests categories that already exist"}>
            <Toggle checked={cfg.allowNewCategories} onChange={(v) => patch({ allowNewCategories: v })} label="Allow AI to create new categories" />
          </Row>
          <Row label="Never overwrite manually assigned categories" hint="Recommended">
            <Toggle checked={cfg.neverOverwriteManual} onChange={(v) => patch({ neverOverwriteManual: v })} label="Never overwrite manually assigned categories" />
          </Row>
          <Row label="Categorise new bookmarks automatically" hint="Runs when a bookmark is saved without a category" last>
            <Toggle checked={cfg.autoCategoriseNew} onChange={(v) => patch({ autoCategoriseNew: v })} label="Categorise new bookmarks automatically" />
          </Row>
        </div>
        <div className="flex flex-col gap-2">
          <span className="text-xs font-medium text-ink-2">Excluded from AI</span>
          <div className="flex flex-wrap gap-1.5">
            {top.map((c) => {
              const on = cfg.excludedCategoryIds.includes(c.id);
              return (
                <button
                  key={c.id}
                  onClick={() => patch({ excludedCategoryIds: on ? cfg.excludedCategoryIds.filter((x) => x !== c.id) : [...cfg.excludedCategoryIds, c.id] })}
                  aria-pressed={on}
                  className={clsx("inline-flex h-7 items-center gap-1.5 rounded-full border px-2.5 text-xs font-medium", on ? "border-transparent" : "border-line text-ink-3 hover:text-ink")}
                  style={on ? { background: tint(c.color, 0.12), color: c.color } : undefined}
                >
                  {on && <X className="size-3" />} {c.name}
                </button>
              );
            })}
            {!top.length && <span className="text-xs text-ink-3">Create categories first.</span>}
          </div>
          <span className="text-xs text-ink-3">AI never sees excluded categories, so it can't suggest them.</span>
        </div>
      </Card>

      <Card title="Usage" sub={usage.lastUsedAt ? `Last used ${formatRelative(usage.lastUsedAt)}` : "Not used yet"}>
        <div className="grid grid-cols-4 gap-2.5">
          {[
            [usage.requests, "Requests"],
            [usage.suggestions, "Suggestions"],
            [usage.accepted, "Accepted"],
            [usage.errors, "Errors"],
          ].map(([n, l]) => (
            <div key={l} className="rounded-[10px] bg-subtle px-3.5 py-2.5">
              <p className="text-xl font-semibold tabular-nums">{Number(n).toLocaleString()}</p>
              <p className="text-xs text-ink-2">{l}</p>
            </div>
          ))}
        </div>
        {usage.lastError && <p className="text-xs text-ink-3">Last error: {usage.lastError}</p>}
      </Card>
    </>
  );
}
