import {
  Bell,
  ChevronRight,
  Clock,
  Download,
  Folder,
  Inbox,
  LayoutGrid,
  PanelLeft,
  Plus,
  Search,
  Settings,
  ShieldAlert,
  Sparkles,
  Star,
  Trash2,
} from "lucide-react";
import clsx from "clsx";
import type { ReactNode } from "react";
import logo from "../../assets/logo.png";
import { useCategories, useCounts } from "../../hooks/queries";
import type { Category, LibraryView } from "../../lib/types";
import { lighten } from "../../lib/colors";
import { useUi, type Route } from "../../store/ui";
import { CountBadge } from "../ui/Badges";
import { Button, IconButton } from "../ui/Button";
import { Kbd } from "../ui/Field";

function isActive(route: Route, view: LibraryView, categoryId?: string) {
  return route.page === "library" && route.view === view && (view !== "category" || route.categoryId === categoryId);
}

function NavItem({ icon, label, count, active, onClick, tone, soon }: { icon: ReactNode; label: string; count?: number; active?: boolean; onClick: () => void; tone?: "danger"; soon?: boolean }) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-current={active ? "page" : undefined}
      className={clsx(
        "flex h-9 w-full items-center gap-2.5 rounded-lg px-2.5 text-left text-sm font-medium transition-colors [&>svg]:size-[18px] [&>svg]:shrink-0",
        active ? "bg-brand-50 text-brand-700 [&>svg]:text-brand-600" : "text-ink-2 hover:bg-muted hover:text-ink",
      )}
    >
      {icon}
      <span className="flex-1 truncate">{label}</span>
      {soon && <span className="rounded-md bg-muted px-1.5 text-[11px] text-ink-3">Soon</span>}
      {count !== undefined && <CountBadge n={count} active={active} tone={tone} />}
    </button>
  );
}

function FolderGlyph({ color }: { color: string }) {
  return (
    <svg width="18" height="14" viewBox="0 0 120 88" aria-hidden className="shrink-0">
      <path fill={color} d="M0 8a8 8 0 0 1 8-8h30c3 0 5 1 7 3l6 6h61a8 8 0 0 1 8 8v63a8 8 0 0 1-8 8H8a8 8 0 0 1-8-8z" />
      <rect y="18" width="120" height="70" rx="8" fill={lighten(color, 0.45)} />
    </svg>
  );
}

function CategoryTree({ categories }: { categories: Category[] }) {
  const { route, navigate, openDialog } = useUi();
  const top = categories.filter((c) => !c.parentId);
  const children = (id: string) => categories.filter((c) => c.parentId === id);
  const total = (c: Category): number => c.bookmarkCount + children(c.id).reduce((n, k) => n + total(k), 0);

  const render = (c: Category, depth: number): ReactNode => {
    const active = isActive(route, "category", c.id);
    return (
      <div key={c.id}>
        <button
          type="button"
          onClick={() => navigate({ page: "library", view: "category", categoryId: c.id })}
          onDoubleClick={() => openDialog({ kind: "category", category: c })}
          aria-current={active ? "page" : undefined}
          className={clsx(
            "flex h-[34px] w-full items-center gap-2.5 rounded-lg pr-2.5 text-left transition-colors",
            active ? "bg-brand-50" : "hover:bg-muted",
          )}
          style={{ paddingLeft: depth === 0 ? 10 : 10 + depth * 24 }}
        >
          {depth === 0 ? <FolderGlyph color={c.color} /> : <span className="size-1.5 shrink-0 rounded-full" style={{ background: c.color }} />}
          <span className={clsx("flex-1 truncate", depth === 0 ? "text-sm font-medium" : "text-[13px] text-ink-2", active && "text-brand-700")}>{c.name}</span>
          <span className="text-xs font-medium text-ink-3 tabular-nums">{total(c)}</span>
        </button>
        {children(c.id).map((k) => render(k, depth + 1))}
      </div>
    );
  };

  return <>{top.map((c) => render(c, 0))}</>;
}

export function Sidebar({ onSearchFocus }: { onSearchFocus: () => void }) {
  const { route, navigate, openDialog, toggleSidebar } = useUi();
  const { data: counts } = useCounts();
  const { data: categories = [] } = useCategories();
  const go = (view: LibraryView) => () => navigate({ page: "library", view });

  return (
    <aside className="flex h-full w-[264px] shrink-0 flex-col gap-1 border-r border-line bg-subtle px-3 pt-3 pb-3">
      <div className="flex items-center gap-2.5 px-1.5 py-1" data-tauri-drag-region>
        <button onClick={() => navigate({ page: "dashboard" })} className="flex flex-1 items-center gap-2.5" aria-label="Go to dashboard">
          <img src={logo} alt="" className="size-[30px] rounded-lg" draggable={false} />
          <span className="text-[17px] font-semibold tracking-tight">Magpie</span>
        </button>
        <IconButton label="Notifications">
          <Bell />
        </IconButton>
        <IconButton label="Collapse sidebar" onClick={toggleSidebar}>
          <PanelLeft />
        </IconButton>
      </div>

      <Button variant="primary" className="mt-2.5 h-10 w-full" icon={<Plus />} onClick={() => openDialog({ kind: "add-bookmark" })}>
        Add Bookmark
      </Button>

      <button
        onClick={onSearchFocus}
        className="mt-1.5 flex h-9 w-full items-center gap-2 rounded-[10px] border border-line bg-canvas px-3 text-sm text-ink-3 hover:border-line-strong"
      >
        <Search className="size-4" />
        <span className="flex-1 text-left">Search</span>
        <Kbd>Ctrl K</Kbd>
      </button>

      {counts && counts.uncategorized > 0 && (
        <button
          onClick={go("uncategorized")}
          className="mt-2.5 flex items-center gap-2.5 rounded-xl border border-line bg-canvas px-3 py-2.5 text-left hover:border-line-strong"
        >
          <span className="grid size-8 place-items-center rounded-[9px] bg-brand-100 text-brand-600">
            <Sparkles className="size-4" />
          </span>
          <span className="min-w-0 flex-1">
            <span className="block text-[13px] font-medium">{counts.uncategorized.toLocaleString()} to organise</span>
            <span className="block text-xs text-ink-3">Uncategorized bookmarks</span>
          </span>
          <ChevronRight className="size-4 text-ink-3" />
        </button>
      )}

      <nav className="mt-3 flex flex-col gap-0.5" aria-label="Library">
        <NavItem icon={<LayoutGrid />} label="All Bookmarks" count={counts?.all ?? 0} active={isActive(route, "all")} onClick={go("all")} />
        <NavItem icon={<Star />} label="Favourites" count={counts?.favourites ?? 0} active={isActive(route, "favourites")} onClick={go("favourites")} />
        <NavItem icon={<Clock />} label="Recently Added" count={counts?.recent ?? 0} active={isActive(route, "recent")} onClick={go("recent")} />
        <NavItem icon={<Inbox />} label="Uncategorized" count={counts?.uncategorized ?? 0} active={isActive(route, "uncategorized")} onClick={go("uncategorized")} />
        <NavItem
          icon={<ShieldAlert />}
          label="Security Alerts"
          count={counts?.securityAlerts ?? 0}
          tone="danger"
          active={isActive(route, "security")}
          onClick={go("security")}
        />
      </nav>

      <div className="mt-3.5 flex items-center px-2.5 py-1">
        <span className="flex-1 text-[11px] font-semibold tracking-[0.04em] text-ink-3">CATEGORIES</span>
        <IconButton label="New category" className="size-6" onClick={() => openDialog({ kind: "category" })}>
          <Plus className="!size-4" />
        </IconButton>
      </div>
      <div className="-mx-1 flex min-h-0 flex-1 flex-col gap-0.5 overflow-y-auto px-1">
        {categories.length === 0 ? (
          <button
            onClick={() => openDialog({ kind: "category" })}
            className="flex h-9 items-center gap-2.5 rounded-lg border border-dashed border-line-strong px-2.5 text-[13px] text-ink-3 hover:text-ink"
          >
            <Folder className="size-4" /> Create your first category
          </button>
        ) : (
          <CategoryTree categories={categories} />
        )}
      </div>

      <div className="my-1 h-px bg-line" />
      <NavItem icon={<Download />} label="Import Bookmarks" onClick={() => openDialog({ kind: "import" })} />
      <NavItem icon={<Trash2 />} label="Trash" count={counts?.trash || undefined} active={isActive(route, "trash")} onClick={go("trash")} />
      <NavItem icon={<Settings />} label="Settings" active={route.page === "settings"} onClick={() => navigate({ page: "settings", section: "general" })} />
    </aside>
  );
}
