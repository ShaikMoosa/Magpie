// Development-only stand-in for the Rust backend, used when the UI runs in a
// plain browser (`npm run dev`) instead of inside Tauri. Keeps data in memory
// and implements just enough of each command to review the UI.

import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import type { Bookmark, BookmarkQuery, Category, CategoryInput, NewBookmark, UpdateBookmark } from "../lib/types";

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
function add(url: string, title: string, description: string | null, categoryId: string | null, hoursAgo: number, extra: Partial<Bookmark> = {}) {
  const domain = new URL(url).hostname.replace(/^www\./, "");
  marks.push({
    id: id(), url, title, description, domain, faviconPath: null, thumbnailPath: null, categoryId, isFavourite: false,
    sourceBrowser: null, sourceFolder: null, securityStatus: "not_checked", securityReasons: [], securitySource: "Local heuristics",
    securityCheckedAt: iso(hoursAgo), tags: [], createdAt: iso(hoursAgo), updatedAt: iso(hoursAgo), lastOpenedAt: null, openCount: 0, deletedAt: null, ...extra,
  });
}
add("https://www.figma.com/community/ui-kits", "Figma Community — UI Kits", "Thousands of free UI kits, wireframes and design systems shared by designers around the world.", uiKits.id, 2, { isFavourite: true, openCount: 48 });
add("https://react.dev/learn", "React — Learn React", "The library for web and native user interfaces. Learn React step by step with interactive examples.", dev.id, 5, { openCount: 41 });
add("https://v2.tauri.app/start/", "Tauri 2 — Create small, fast desktop apps", "Build smaller, faster and more secure desktop and mobile applications with a web frontend.", dev.id, 9, { isFavourite: true });
add("https://paypa1-secure-login.com/verify", "Account verification required", "Your account has been limited. Verify your identity within 24 hours to restore access.", null, 12, {
  securityStatus: "suspicious", securityReasons: ["Looks like paypal but is a different site"],
});
add("https://mobbin.com", "Mobbin — Design Patterns", "The world's largest library of mobile and web app screenshots.", design.id, 30, { isFavourite: true });
add("https://lucide.dev", "Lucide Icons", "Beautiful & consistent icon toolkit made by the community.", design.id, 40);
add("https://www.nngroup.com/articles/", "Nielsen Norman — UX research methods", null, research.id, 80);
add("https://paulgraham.com/greatwork.html", "How to Do Great Work", null, reading.id, 120);
add("https://excalidraw.com", "Excalidraw — Virtual whiteboard", null, null, 3);

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
  mockWindows("main");
  mockIPC((cmd, payload) => {
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
      default:
        if (cmd.startsWith("plugin:window|")) return null;
        console.warn("mock: unhandled command", cmd);
        return null;
    }
  });
}
