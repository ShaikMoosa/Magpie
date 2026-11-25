// Development-only stand-in for the Rust backend, used when the UI runs in a
// plain browser (`npm run dev`) instead of inside Tauri. Keeps data in memory
// and implements just enough of each command to review the UI.

import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import type { AiConfig, AiSuggestion, Bookmark, BookmarkQuery, Category, CategoryInput, NewBookmark, UpdateBookmark } from "../lib/types";

const now = Date.now();
const iso = (hoursAgo: number) => new Date(now - hoursAgo * 3_600_000).toISOString();
let seq = 1;
const id = () => `mock-${seq++}`;

const cats: Category[] = [];
function cat(name: string, color: string, parentId: string | null = null, description: string | null = null) {
  const c: Category = { id: id(), name, description, color, icon: "folder", parentId, sortOrder: cats.length, createdAt: iso(500), updatedAt: iso(500), bookmarkCount: 0 };
  cats.push(c);
  return c;
}
const design = cat("Design", "#7C3AED", null, "UI kits, icon libraries and inspiration");
const uiKits = cat("UI Kits", "#7C3AED", design.id);
cat("Icon Libraries", "#7C3AED", design.id);
const dev = cat("Development", "#2563EB", null, "Docs & repos");
const research = cat("Research", "#0D9488");
cat("Productivity", "#EA580C");
const reading = cat("Reading List", "#DB2777");

const marks: Bookmark[] = [];
const overrides: { domain: string; createdAt: string }[] = [];
function add(url: string, title: string, description: string | null, categoryId: string | null, hoursAgo: number, extra: Partial<Bookmark> = {}) {
  const domain = new URL(url).hostname.replace(/^www\./, "");
  marks.push({
    id: id(), url, title, description, domain, faviconPath: null, thumbnailPath: null, categoryId, isFavourite: false,
    sourceBrowser: null, sourceFolder: null, securityStatus: "not_checked", securityReasons: [], securitySource: "Local heuristics",
    securityCheckedAt: iso(hoursAgo), tags: [], createdAt: iso(hoursAgo), updatedAt: iso(hoursAgo), lastOpenedAt: null, openCount: 0, deletedAt: null, metadataStatus: "ok", ...extra,
  });
}
add("https://www.figma.com/community/ui-kits", "Figma Community — UI Kits", "Thousands of free UI kits, wireframes and design systems shared by designers around the world.", uiKits.id, 2, { isFavourite: true, openCount: 48 });
add("https://react.dev/learn", "React — Learn React", "The library for web and native user interfaces. Learn React step by step with interactive examples.", dev.id, 5, { openCount: 41 });
add("https://v2.tauri.app/start/", "Tauri 2 — Create small, fast desktop apps", "Build smaller, faster and more secure desktop and mobile applications with a web frontend.", dev.id, 9, { isFavourite: true });
add("https://paypa1-secure-login.com/verify", "Account verification required", "Your account has been limited. Verify your identity within 24 hours to restore access.", null, 12, {
  securityStatus: "suspicious", securityReasons: ["Looks like paypal but is a different site"],
});
add("https://mobbin.com", "Mobbin — Design Patterns", "The world's largest library of mobile and web app screenshots.", design.id, 30, { isFavourite: true });
add("https://lucide.dev", "Lucide Icons", "Beautiful & consistent icon toolkit made by the community.", design.id, 40, { tags: ["icons", "svg"], sourceBrowser: "Chrome" });
add("https://www.nngroup.com/articles/", "Nielsen Norman — UX research methods", null, research.id, 80);
add("https://paulgraham.com/greatwork.html", "How to Do Great Work", null, reading.id, 120);
add("https://excalidraw.com", "Excalidraw — Virtual whiteboard", null, null, 3);

add("https://www.awwwards.com/websites/", "Awwwards — Website inspiration", "The best web design and development inspiration.", null, 6);
add("https://docs.rs/serde", "serde — Rust docs", "A generic serialization/deserialization framework.", null, 7);
add("https://arxiv.org/abs/2401.00001", "A survey of retrieval methods", null, null, 26);

// --- Data & Storage (mock) ---
let previewsBytes = 34_100_000;
let backupCfg = { daily: true, keep: 7 };
const mockBackups = [
  { name: "magpie-a-auto.db", kind: "auto", createdAt: iso(3), size: 12_400_000 },
  { name: "magpie-b-auto.db", kind: "auto", createdAt: iso(27), size: 12_300_000 },
  { name: "magpie-c-before-import.db", kind: "before-import", createdAt: iso(140), size: 11_800_000 },
];

// --- General (mock) ---
let behaviourCfg = { minimiseToTray: false, refreshDays: 30, cacheLimitMb: 500 };
let autostart = false;

// --- Security scanners (mock) ---
let secCfg = {
  enabled: false, google: false, urlhaus: false, virustotal: false, schedule: "on_save_weekly", consent: "ask",
  stripTracking: true, warnSuspicious: true, blockDangerous: true, expiryDays: 7,
};
const scanKeys = new Set<string>();
const scanUsage = { month: "2026-11", google: 0, urlhaus: 0, virustotal: 0, lastScanAt: null as string | null, lastError: null as string | null };

// --- AI (mock) ---
let aiConfig: AiConfig = {
  enabled: false, provider: "anthropic", model: "claude-opus-5-5", endpoint: "", mode: "suggestion",
  allowNewCategories: false, neverOverwriteManual: true, autoCategoriseNew: true, excludedCategoryIds: [],
};
const aiKeys = new Set<string>();
const aiUsage = { requests: 0, suggestions: 0, accepted: 0, errors: 0, lastUsedAt: null as string | null, lastError: null as string | null };
const pending = new Map<string, AiSuggestion & { createdAt: string }>();
const rejected = new Set<string>();
function guess(url: string, title: string): AiSuggestion {
  const t = `${url} ${title}`.toLowerCase();
  const pick = (name: string) => cats.find((c) => c.name === name)!.id;
  const [categoryId, tags, reason] =
    /figma|dribbble|awwwards|design|ui/.test(t) ? [pick("Design"), ["inspiration"], "Design inspiration gallery"]
    : /docs|github|rust|react|api|dev/.test(t) ? [pick("Development"), ["docs"], "Developer documentation"]
    : /arxiv|research|study|survey/.test(t) ? [pick("Research"), ["paper"], "Academic paper"]
    : /draw|notion|todo|whiteboard/.test(t) ? [pick("Productivity"), ["tools"], "Productivity tool"]
    : [null, [], "Nothing fits well"];
  aiUsage.requests++;
  aiUsage.lastUsedAt = new Date().toISOString();
  return { categoryId: categoryId as string | null, newCategory: null, tags: tags as string[], confidence: categoryId ? 0.86 : 0.3, reason: reason as string, provider: aiConfig.provider, model: aiConfig.model };
}

function counts() {
  const live = marks.filter((b) => !b.deletedAt);
  const recentSince = now - 30 * 86_400_000;
  return {
    all: live.length,
    favourites: live.filter((b) => b.isFavourite).length,
    recent: live.filter((b) => +new Date(b.createdAt) >= recentSince).length,
    uncategorized: live.filter((b) => !b.categoryId).length,
    securityAlerts: live.filter((b) => b.securityStatus === "suspicious" || b.securityStatus === "dangerous").length,
    trash: marks.filter((b) => b.deletedAt).length,
  };
}

function withCounts() {
  return cats.map((c) => ({ ...c, bookmarkCount: marks.filter((b) => !b.deletedAt && b.categoryId === c.id).length }));
}

function descendants(cid: string): string[] {
  return cats.filter((c) => c.parentId === cid).flatMap((c) => [c.id, ...descendants(c.id)]);
}

function list(q: BookmarkQuery) {
  let r = marks.filter((b) => (q.view === "trash" ? b.deletedAt : !b.deletedAt));
  if (q.view === "favourites" || q.favouritesOnly) r = r.filter((b) => b.isFavourite);
  if (q.domain) r = r.filter((b) => b.domain === q.domain);
  if (q.security) r = r.filter((b) => b.securityStatus === q.security);
  if (q.source) r = r.filter((b) => (b.sourceBrowser ?? "manual") === q.source);
  if (q.tags?.length) r = r.filter((b) => q.tags!.every((t) => b.tags.includes(t)));
  if (q.addedWithinDays) r = r.filter((b) => +new Date(b.createdAt) >= now - q.addedWithinDays! * 86_400_000);
  if (q.view === "uncategorized") r = r.filter((b) => !b.categoryId);
  if (q.view === "security") r = r.filter((b) => b.securityStatus === "suspicious" || b.securityStatus === "dangerous");
  if (q.view === "category" && q.categoryId) {
    const ids = new Set([q.categoryId, ...descendants(q.categoryId)]);
    r = r.filter((b) => b.categoryId && ids.has(b.categoryId));
  }
  const words = (q.search ?? "").toLowerCase().split(/\W+/).filter(Boolean);
  if (words.length) {
    r = r.filter((b) => {
      const hay = [b.title, b.description, b.url, b.domain, cats.find((c) => c.id === b.categoryId)?.name, ...b.tags].join(" ").toLowerCase();
      return words.every((w) => hay.split(/\W+/).some((t) => t.startsWith(w)));
    });
  }
  const by = q.sort ?? "recently_added";
  r = [...r].sort((a, b) =>
    by === "title_asc" ? a.title.localeCompare(b.title)
    : by === "title_desc" ? b.title.localeCompare(a.title)
    : by === "domain" ? a.domain.localeCompare(b.domain)
    : by === "most_opened" ? b.openCount - a.openCount
    : +new Date(b.createdAt) - +new Date(a.createdAt),
  );
  return q.limit ? r.slice(0, q.limit) : r;
}

function checkUrl(raw: string, exclude?: string | null) {
  let s = raw.trim();
  if (!s) throw { code: "INVALID_URL", message: "Enter a website address.", existingId: null };
  if (!s.includes("://")) s = `https://${s}`;
  let u: URL;
  try {
    u = new URL(s);
  } catch {
    throw { code: "INVALID_URL", message: "That doesn't look like a web address.", existingId: null };
  }
  if (!/^https?:$/.test(u.protocol) || !u.hostname.includes(".")) throw { code: "INVALID_URL", message: "That doesn't look like a web address.", existingId: null };
  const domain = u.hostname.replace(/^www\./, "");
  const normalized = domain + u.pathname.replace(/\/$/, "") + u.search;
  const dup = marks.find((b) => !b.deletedAt && b.id !== exclude && new URL(b.url).hostname.replace(/^www\./, "") + new URL(b.url).pathname.replace(/\/$/, "") + new URL(b.url).search === normalized);
  const suspicious = /paypa1|-google|\.exe$/.test(u.href);
  return {
    url: u.href, normalized, domain, duplicateOf: dup?.id ?? null,
    securityStatus: suspicious ? "suspicious" : "not_checked",
    securityReasons: [...(suspicious ? ["Looks like a well-known brand but is a different site"] : []), ...(u.protocol === "http:" ? ["Uses an insecure HTTP connection"] : [])],
  };
}

export function installMockBackend() {
  // `?empty` starts with an empty library (shows the first-run welcome screen).
  if (new URLSearchParams(location.search).has("empty")) {
    marks.length = 0;
    cats.length = 0;
    localStorage.removeItem("magpie.prefs");
  }
  mockWindows("main");
  mockIPC(async (cmd, payload) => {
    const a = (payload ?? {}) as Record<string, any>;
    switch (cmd) {
      case "list_bookmarks": return list(a.query);
      case "get_counts": return counts();
      case "check_url": return checkUrl(a.url, a.excludeId);
      case "create_bookmark": {
        const i = a.input as NewBookmark;
        const c = checkUrl(i.url);
        if (c.duplicateOf && !i.allowDuplicate) throw { code: "DUPLICATE_BOOKMARK", message: "This URL is already in your library.", existingId: c.duplicateOf };
        add(c.url, i.title || c.domain, i.description ?? null, i.categoryId ?? null, 0, { isFavourite: !!i.isFavourite, tags: i.tags ?? [], securityStatus: c.securityStatus as Bookmark["securityStatus"], securityReasons: c.securityReasons });
        return marks[marks.length - 1];
      }
      case "update_bookmark": {
        const i = a.input as UpdateBookmark;
        const b = marks.find((x) => x.id === i.id)!;
        Object.assign(b, { ...i, title: i.title || b.domain, updatedAt: new Date().toISOString() });
        return b;
      }
      case "set_favourite": marks.forEach((b) => a.ids.includes(b.id) && (b.isFavourite = a.value)); return a.ids.length;
      case "move_bookmarks": marks.forEach((b) => a.ids.includes(b.id) && (b.categoryId = a.categoryId)); return a.ids.length;
      case "trash_bookmarks": marks.forEach((b) => a.ids.includes(b.id) && (b.deletedAt = new Date().toISOString())); return a.ids.length;
      case "restore_bookmarks": marks.forEach((b) => a.ids.includes(b.id) && (b.deletedAt = null)); return a.ids.length;
      case "purge_bookmarks": {
        const before = marks.length;
        for (let i = marks.length - 1; i >= 0; i--) if (a.ids.includes(marks[i].id) && marks[i].deletedAt) marks.splice(i, 1);
        return before - marks.length;
      }
      case "empty_trash": {
        const before = marks.length;
        for (let i = marks.length - 1; i >= 0; i--) if (marks[i].deletedAt) marks.splice(i, 1);
        return before - marks.length;
      }
      case "open_bookmark": {
        const b = marks.find((x) => x.id === a.id)!;
        if ((b.securityStatus === "suspicious" || b.securityStatus === "dangerous") && !a.confirmed) return { opened: false, bookmark: b };
        b.openCount++;
        b.lastOpenedAt = new Date().toISOString();
        window.open(b.url, "_blank", "noopener");
        return { opened: true, bookmark: b };
      }
      case "get_ai_settings": return { config: aiConfig, keys: [...aiKeys], usage: aiUsage, running: false };
      case "save_ai_config": aiConfig = a.config; return aiConfig;
      case "set_ai_key": aiKeys.add(a.provider); return null;
      case "remove_ai_key": aiKeys.delete(a.provider); return null;
      case "list_ai_models":
        if (!aiKeys.has(a.config.provider)) throw { code: "VALIDATION", message: "Add your API key in Settings › AI." };
        return a.config.provider === "anthropic" ? ["claude-opus-5-5", "claude-sonnet-5-5", "claude-haiku-5-5"] : ["model-large", "model-small"];
      case "suggest_category": {
        await new Promise((r) => setTimeout(r, 700));
        return guess(a.draft.url, a.draft.title ?? "");
      }
      case "categorize_bookmarks": {
        const targets = marks.filter((b) => !b.deletedAt && (a.ids ? a.ids.includes(b.id) : !b.categoryId && !pending.has(b.id) && !rejected.has(b.id)));
        for (const b of targets) {
          const g = guess(b.url, b.title);
          if (aiConfig.mode === "automatic" && g.categoryId && !b.categoryId) b.categoryId = g.categoryId;
          else pending.set(b.id, { ...g, createdAt: new Date().toISOString() });
          if (g.categoryId) aiUsage.suggestions++;
        }
        return targets.length;
      }
      case "list_ai_suggestions":
        return [...pending.entries()].flatMap(([bid, s]) => {
          const bookmark = marks.find((b) => b.id === bid && !b.deletedAt && !b.categoryId);
          return bookmark ? [{ ...s, bookmark }] : [];
        });
      case "accept_ai_suggestions": {
        let n = 0;
        for (const bid of a.ids as string[]) {
          const s = pending.get(bid);
          const b = marks.find((x) => x.id === bid);
          if (!s || !b || !s.categoryId) continue;
          b.categoryId = s.categoryId;
          b.tags = [...new Set([...b.tags, ...s.tags])];
          pending.delete(bid);
          aiUsage.accepted++;
          n++;
        }
        return n;
      }
      case "reject_ai_suggestions":
        for (const bid of a.ids as string[]) {
          pending.delete(bid);
          rejected.add(bid);
        }
        return a.ids.length;
      case "get_storage_info":
        return {
          dbPath: String.raw`C:\Users\you\AppData\Roaming\com.shaikmoosa.magpie\magpie.db`, dataDir: "", backupsDir: String.raw`C:\Users\you\AppData\Roaming\com.shaikmoosa.magpie\backups`,
          databaseBytes: 9_600_000, previewsBytes: previewsBytes, backupsBytes: mockBackups.reduce((n, b) => n + b.size, 0),
          bookmarks: counts().all, categories: cats.length, trash: counts().trash,
        };
      case "reveal_data_folder": return null;
      case "export_library": return a.ids ? a.ids.length : counts().all;
      case "get_backup_config": return backupCfg;
      case "save_backup_config": backupCfg = a.config; return backupCfg;
      case "list_backups": return mockBackups;
      case "create_backup": {
        const b = { name: `magpie-${Date.now()}-manual.db`, kind: "manual", createdAt: new Date().toISOString(), size: 12_600_000 };
        mockBackups.unshift(b);
        return b;
      }
      case "inspect_backup": return { bookmarks: 1248, categories: 9 };
      case "restore_backup": return { bookmarks: 1248, categories: 9 };
      case "clear_preview_cache": { const n = previewsBytes; previewsBytes = 0; return n; }
      case "reset_app_data": marks.length = 0; cats.length = 0; return null;
      case "get_security_settings": return { config: secCfg, keys: [...scanKeys], usage: scanUsage, running: false };
      case "save_security_config": secCfg = a.config; return secCfg;
      case "set_scan_key":
        await new Promise((r) => setTimeout(r, 500));
        if (a.key === "bad") throw { code: "NETWORK", message: "Google Safe Browsing rejected the API key. Check it in Settings › Security." };
        scanKeys.add(a.provider); return null;
      case "remove_scan_key": scanKeys.delete(a.provider); return null;
      case "scan_bookmarks": {
        if (secCfg.consent === "ask" && !a.confirmed) throw { code: "VALIDATION", message: "Confirm before URLs are sent to scanners." };
        const live = marks.filter((b) => !b.deletedAt && (!a.ids || a.ids.includes(b.id)));
        for (const b of live) {
          const bad = b.domain.includes("paypa1");
          b.securityStatus = bad ? "dangerous" : b.securityStatus === "suspicious" ? "suspicious" : "no_threat";
          b.securitySource = "Google Safe Browsing · URLhaus · Local heuristics";
          if (bad) b.securityReasons = ["Google Safe Browsing lists this as phishing or a deceptive site", ...b.securityReasons];
          b.securityCheckedAt = new Date().toISOString();
        }
        scanUsage.google += live.length;
        scanUsage.lastScanAt = new Date().toISOString();
        return live.length;
      }
      case "open_private": {
        const b = marks.find((x) => x.id === a.id)!;
        if (!a.confirmed && (b.securityStatus === "suspicious" || b.securityStatus === "dangerous")) return { opened: false, bookmark: b };
        return { opened: true, bookmark: b };
      }
      case "add_tags":
        for (const b of marks.filter((x) => a.ids.includes(x.id))) b.tags = [...new Set([...b.tags, ...a.tags])].sort();
        return a.ids.length;
      case "get_general_settings": return { behaviour: behaviourCfg, autostart };
      case "save_app_behaviour": behaviourCfg = a.behaviour; return behaviourCfg;
      case "set_autostart": autostart = a.enabled; return autostart;
      case "list_categories": return withCounts();
      case "create_category": {
        const i = a.input as CategoryInput;
        if (cats.some((c) => c.parentId === i.parentId && c.name.toLowerCase() === i.name.trim().toLowerCase()))
          throw { code: "VALIDATION_ERROR", message: "A category with this name already exists here.", existingId: null };
        return cat(i.name.trim(), i.color.toUpperCase(), i.parentId, i.description);
      }
      case "update_category": {
        const c = cats.find((x) => x.id === a.input.id)!;
        Object.assign(c, a.input);
        return c;
      }
      case "delete_category": {
        const { id: cid, moveTo, keepSubcategories } = a.input;
        const subs = descendants(cid);
        const emptied = new Set([cid, ...(keepSubcategories ? [] : subs)]);
        marks.forEach((b) => b.categoryId && emptied.has(b.categoryId) && (b.categoryId = moveTo));
        if (keepSubcategories) cats.forEach((c) => c.parentId === cid && (c.parentId = null));
        for (let i = cats.length - 1; i >= 0; i--) if (emptied.has(cats[i].id)) cats.splice(i, 1);
        return null;
      }
      case "reorder_categories": return null;
      case "previews_dir": return "";
      case "get_facets": {
        const live = marks.filter((b) => !b.deletedAt);
        const tally = (vals: string[]) => Object.entries(vals.reduce<Record<string, number>>((m, v) => ((m[v] = (m[v] ?? 0) + 1), m), {})).map(([value, count]) => ({ value, count })).sort((x, y) => y.count - x.count);
        return { domains: tally(live.map((b) => b.domain)), tags: tally(live.flatMap((b) => b.tags)), sources: tally(live.map((b) => b.sourceBrowser ?? "manual")), security: tally(live.map((b) => b.securityStatus)) };
      }
      case "rescan_bookmarks": {
        const t = marks.filter((b) => !b.deletedAt && (!a.ids || a.ids.includes(b.id)));
        return { scanned: t.length, suspicious: t.filter((b) => b.securityStatus === "suspicious").length };
      }
      case "mark_safe": {
        const domains = new Set(marks.filter((b) => a.ids.includes(b.id)).map((b) => b.domain));
        domains.forEach((d) => overrides.push({ domain: d, createdAt: new Date().toISOString() }));
        marks.forEach((b) => domains.has(b.domain) && Object.assign(b, { securityStatus: "not_checked", securityReasons: ["You marked this site as safe"] }));
        return domains.size;
      }
      case "list_overrides": return overrides;
      case "remove_override": {
        const i = overrides.findIndex((o) => o.domain === a.domain);
        if (i >= 0) overrides.splice(i, 1);
        marks.forEach((b) => b.domain === a.domain && b.domain.startsWith("paypa1") && Object.assign(b, { securityStatus: "suspicious", securityReasons: ["Looks like paypal but is a different site"] }));
        return null;
      }
      case "refresh_metadata": return null;
      case "fetch_metadata": {
        const host = new URL(a.url).hostname.replace(/^www\./, "");
        return { title: `${host[0].toUpperCase()}${host.slice(1)} — Example page title`, description: "A short description fetched from the page's metadata.", siteName: host, image: null, favicon: null, finalUrl: a.url };
      }
      case "detect_browsers":
        return [
          { browser: "chrome", dir: "Default", name: "Personal", bookmarkCount: 1068 },
          { browser: "chrome", dir: "Profile 1", name: "Work", bookmarkCount: 0 },
          { browser: "edge", dir: "Default", name: "Profile 1", bookmarkCount: 2565 },
        ];
      case "plugin:dialog|open": return "C:\\Users\\you\\Downloads\\bookmarks_25_11_2026.html";
      case "preview_import": {
        const S = "\u001f";
        return {
          sourceLabel: a.source.kind === "html" ? "bookmarks_25_11_2026.html" : `${a.source.browser === "chrome" ? "Chrome" : "Edge"} — Personal`,
          total: 1068, folders: 23, valid: 1063, invalid: 5, duplicatesInFile: 27, alreadyInLibrary: 12, newCount: 1024,
          folderTree: [
            { key: "Bookmarks bar", name: "Bookmarks bar", depth: 0, count: 18 },
            { key: `Bookmarks bar${S}Design`, name: "Design", depth: 1, count: 128 },
            { key: `Bookmarks bar${S}Design${S}Fonts`, name: "Fonts", depth: 2, count: 34 },
            { key: `Bookmarks bar${S}Dev docs`, name: "Dev docs", depth: 1, count: 164 },
            { key: `Bookmarks bar${S}Reading`, name: "Reading", depth: 1, count: 96 },
            { key: "Other bookmarks", name: "Other bookmarks", depth: 0, count: 590 },
            { key: "Mobile bookmarks", name: "Mobile bookmarks", depth: 0, count: 38 },
          ],
        };
      }
      case "start_import": {
        const n = 6;
        for (let i = 0; i < n; i++) add(`https://example-${seq}.dev/`, `Imported bookmark ${i + 1}`, null, null, 2000 + i, { sourceBrowser: "Chrome" });
        return { total: 1068, imported: 1024, duplicatesSkipped: 39, invalidSkipped: 5, categoriesCreated: 4 };
      }
      default:
        if (cmd.startsWith("plugin:window|")) return null;
        console.warn("mock: unhandled command", cmd);
        return null;
    }
  });
}
