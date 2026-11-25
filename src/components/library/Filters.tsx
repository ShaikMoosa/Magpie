import * as Popover from "@radix-ui/react-popover";
import { Check, ChevronDown, Search, Star, X } from "lucide-react";
import clsx from "clsx";
import { useState, type ReactNode } from "react";
import { useFacets } from "../../hooks/queries";
import type { Facet, LibraryView, SecurityStatus } from "../../lib/types";

export interface FilterState {
  favouritesOnly: boolean;
  domain: string | null;
  tags: string[];
  security: SecurityStatus | null;
  source: string | null;
  addedWithinDays: number | null;
}

export const NO_FILTERS: FilterState = { favouritesOnly: false, domain: null, tags: [], security: null, source: null, addedWithinDays: null };

export function activeFilterCount(f: FilterState) {
  return [f.favouritesOnly, f.domain, f.tags.length, f.security, f.source, f.addedWithinDays].filter(Boolean).length;
}

const SECURITY_LABELS: Record<SecurityStatus, string> = {
  suspicious: "Suspicious",
  dangerous: "Dangerous",
  not_checked: "Not checked",
  no_threat: "No threat",
};
const SOURCE_LABELS: Record<string, string> = { manual: "Added in Magpie", Chrome: "Chrome", Edge: "Edge", "HTML file": "HTML file" };
const DATES: { days: number; label: string }[] = [
  { days: 1, label: "Last 24 hours" },
  { days: 7, label: "Last 7 days" },
  { days: 30, label: "Last 30 days" },
  { days: 90, label: "Last 90 days" },
  { days: 365, label: "Last year" },
];

function Chip({ label, value, active, onClear, children }: { label: string; value: string | null; active: boolean; onClear: () => void; children: ReactNode }) {
  const [open, setOpen] = useState(false);
  return (
    <Popover.Root open={open} onOpenChange={setOpen}>
      <div
        className={clsx(
          "inline-flex h-9 items-center rounded-[9px] border text-[13px]",
          active ? "border-brand-400 bg-brand-50 text-brand-700" : "border-line-strong bg-canvas text-ink-2 hover:bg-subtle",
        )}
      >
        <Popover.Trigger className="flex h-full items-center gap-1.5 pr-1.5 pl-2.5">
          <span>{label}</span>
          <span className={clsx("max-w-32 truncate font-medium", active ? "text-brand-700" : "text-ink")}>{value ?? "Any"}</span>
          {!active && <ChevronDown className="size-3.5 text-ink-3" />}
        </Popover.Trigger>
        {active && (
          <button aria-label={`Clear ${label.replace(":", "")} filter`} onClick={onClear} className="grid h-full place-items-center pr-2 pl-0.5 text-brand-600">
            <X className="size-3.5" />
          </button>
        )}
      </div>
      <Popover.Portal>
        <Popover.Content align="start" sideOffset={4} className="z-50 w-64 rounded-xl border border-line bg-canvas p-1.5 shadow-lg" onClick={() => setOpen(false)}>
          {children}
        </Popover.Content>
      </Popover.Portal>
    </Popover.Root>
  );
}

function Option({ selected, onSelect, children, count }: { selected: boolean; onSelect: () => void; children: ReactNode; count?: number }) {
  return (
    <button onClick={onSelect} className="flex h-8 w-full items-center gap-2 rounded-[7px] px-2.5 text-left text-[13px] hover:bg-muted">
      <span className="min-w-0 flex-1 truncate">{children}</span>
      {count !== undefined && <span className="text-xs text-ink-3 tabular-nums">{count}</span>}
      <Check className={clsx("size-4 text-brand-600", !selected && "invisible")} />
    </button>
  );
}

function SearchableList({ items, isSelected, onSelect, placeholder, empty }: { items: Facet[]; isSelected: (v: string) => boolean; onSelect: (v: string) => void; placeholder: string; empty: string }) {
  const [q, setQ] = useState("");
  const shown = items.filter((f) => f.value.toLowerCase().includes(q.trim().toLowerCase())).slice(0, 50);
  return (
    <div onClick={(e) => e.stopPropagation()}>
      <label className="mb-1 flex h-8 items-center gap-2 rounded-[7px] border border-line px-2">
        <Search className="size-3.5 text-ink-3" />
        <input autoFocus value={q} onChange={(e) => setQ(e.target.value)} placeholder={placeholder} className="min-w-0 flex-1 bg-transparent text-[13px] outline-none" />
      </label>
      <div className="max-h-64 overflow-y-auto">
        {shown.length === 0 ? (
          <p className="px-2.5 py-2 text-xs text-ink-3">{empty}</p>
        ) : (
          shown.map((f) => (
            <Option key={f.value} selected={isSelected(f.value)} onSelect={() => onSelect(f.value)} count={f.count}>
              {f.value}
            </Option>
          ))
        )}
      </div>
    </div>
  );
}

export function FilterBar({ view, value, onChange }: { view: LibraryView; value: FilterState; onChange: (f: FilterState) => void }) {
  const { data: facets } = useFacets();
  const set = (patch: Partial<FilterState>) => onChange({ ...value, ...patch });
  const showSecurity = view !== "security";
  return (
    <>
      {view !== "favourites" && view !== "trash" && (
        <button
          onClick={() => set({ favouritesOnly: !value.favouritesOnly })}
          aria-pressed={value.favouritesOnly}
          className={clsx(
            "inline-flex h-9 items-center gap-1.5 rounded-[9px] border px-2.5 text-[13px]",
            value.favouritesOnly ? "border-brand-400 bg-brand-50 text-brand-700" : "border-line-strong bg-canvas text-ink-2 hover:bg-subtle",
          )}
        >
          <Star className={clsx("size-3.5", value.favouritesOnly && "fill-current")} /> Favourites {value.favouritesOnly && <X className="size-3.5" />}
        </button>
      )}
      <Chip label="Domain:" value={value.domain} active={!!value.domain} onClear={() => set({ domain: null })}>
        <SearchableList
          items={facets?.domains ?? []}
          isSelected={(v) => v === value.domain}
          onSelect={(v) => set({ domain: v === value.domain ? null : v })}
          placeholder="Find a domain…"
          empty="No matching domains"
        />
      </Chip>
      <Chip label="Tags:" value={value.tags.length ? value.tags.map((t) => `#${t}`).join(" ") : null} active={value.tags.length > 0} onClear={() => set({ tags: [] })}>
        <SearchableList
          items={facets?.tags ?? []}
          isSelected={(v) => value.tags.includes(v)}
          onSelect={(v) => set({ tags: value.tags.includes(v) ? value.tags.filter((t) => t !== v) : [...value.tags, v] })}
          placeholder="Find a tag…"
          empty="No tags yet — add them when editing a bookmark"
        />
      </Chip>
      {showSecurity && (
        <Chip label="Security:" value={value.security ? SECURITY_LABELS[value.security] : null} active={!!value.security} onClear={() => set({ security: null })}>
          {(Object.keys(SECURITY_LABELS) as SecurityStatus[]).map((s) => (
            <Option key={s} selected={value.security === s} onSelect={() => set({ security: value.security === s ? null : s })} count={facets?.security.find((f) => f.value === s)?.count ?? 0}>
              {SECURITY_LABELS[s]}
            </Option>
          ))}
        </Chip>
      )}
      <Chip label="Source:" value={value.source ? (SOURCE_LABELS[value.source] ?? value.source) : null} active={!!value.source} onClear={() => set({ source: null })}>
        {(facets?.sources ?? []).map((f) => (
          <Option key={f.value} selected={value.source === f.value} onSelect={() => set({ source: value.source === f.value ? null : f.value })} count={f.count}>
            {SOURCE_LABELS[f.value] ?? f.value}
          </Option>
        ))}
      </Chip>
      {view !== "recent" && (
        <Chip label="Added:" value={DATES.find((d) => d.days === value.addedWithinDays)?.label ?? null} active={!!value.addedWithinDays} onClear={() => set({ addedWithinDays: null })}>
          {DATES.map((d) => (
            <Option key={d.days} selected={value.addedWithinDays === d.days} onSelect={() => set({ addedWithinDays: value.addedWithinDays === d.days ? null : d.days })}>
              {d.label}
            </Option>
          ))}
        </Chip>
      )}
      {activeFilterCount(value) > 1 && (
        <button onClick={() => onChange(NO_FILTERS)} className="h-9 px-1.5 text-[13px] font-medium text-brand-600 hover:text-brand-700">
          Clear all
        </button>
      )}
    </>
  );
}
