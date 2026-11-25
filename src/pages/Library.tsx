import * as DM from "@radix-ui/react-dropdown-menu";
import {
  ArrowUpDown,
  Check,
  ChevronDown,
  ChevronRight,
  Clock,
  Folder,
  FolderInput,
  Inbox,
  LayoutGrid,
  List,
  Pencil,
  Plus,
  Rows3,
  Search,
  Sparkles,
  ShieldAlert,
  ShieldCheck,
  Star,
  Trash2,
  X,
} from "lucide-react";
import clsx from "clsx";
import { forwardRef, useEffect, useMemo, useState, type ReactNode } from "react";
import { BookmarkCard, BookmarkCompact, BookmarkRow } from "../components/bookmarks/BookmarkItems";
import { useBookmarkActions } from "../components/bookmarks/useBookmarkActions";
import { TopBar } from "../components/layout/TopBar";
import { activeFilterCount, FilterBar, NO_FILTERS, type FilterState } from "../components/library/Filters";
import { AiReviewPanel } from "../components/library/AiReview";
import { SecurityAlertList, SecurityHeaderAction, SecuritySummary, TrustedSites } from "../components/library/SecurityAlerts";
import { Button } from "../components/ui/Button";
import { EmptyState } from "../components/ui/EmptyState";
import { useAiSettings, useBookmarks, useCategories, useCategorize, useCounts } from "../hooks/queries";
import { tint } from "../lib/colors";
import { plural } from "../lib/format";
import type { Bookmark, Category, LibraryView, Sort, ViewMode } from "../lib/types";
import { usePrefs, useUi } from "../store/ui";

const EMPTY: Bookmark[] = [];

const SORTS: { value: Sort; label: string }[] = [
  { value: "recently_added", label: "Recently added" },
  { value: "recently_modified", label: "Recently modified" },
  { value: "title_asc", label: "Title A–Z" },
  { value: "title_desc", label: "Title Z–A" },
  { value: "domain", label: "Domain" },
  { value: "recently_opened", label: "Recently opened" },
  { value: "most_opened", label: "Most opened" },
];

const META: Record<Exclude<LibraryView, "category">, { title: string; icon: ReactNode; box: string; sub: (n: number) => string }> = {
  all: { title: "All Bookmarks", icon: <LayoutGrid />, box: "bg-brand-50 text-brand-600", sub: (n) => plural(n, "bookmark") },
  favourites: { title: "Favourites", icon: <Star />, box: "bg-warning-bg text-warning", sub: (n) => `${plural(n, "bookmark")} you come back to` },
  recent: { title: "Recently Added", icon: <Clock />, box: "bg-brand-50 text-brand-600", sub: (n) => `${plural(n, "bookmark")} added in the last 30 days` },
  uncategorized: { title: "Uncategorized", icon: <Inbox />, box: "bg-muted text-ink-2", sub: (n) => `${plural(n, "bookmark")} without a category` },
  security: { title: "Security Alerts", icon: <ShieldAlert />, box: "bg-danger-bg text-danger", sub: () => "Flagged by local checks and your security providers. A link without a warning is not guaranteed to be safe." },
  trash: { title: "Trash", icon: <Trash2 />, box: "bg-muted text-ink-2", sub: (n) => `${plural(n, "bookmark")} · kept until you restore them or empty Trash` },
};

export const SearchBox = forwardRef<HTMLInputElement, { placeholder: string }>(function SearchBox({ placeholder }, ref) {
  const { search, setSearch } = useUi();
  return (
    <div className="flex h-9 w-[240px] items-center gap-2 rounded-[10px] border border-line-strong bg-canvas px-3 focus-within:border-brand-500 focus-within:ring-[3px] focus-within:ring-brand-500/15">
      <Search className="size-4 text-ink-3" />
      <input
        ref={ref}
        value={search}
        onChange={(e) => setSearch(e.target.value)}
        onKeyDown={(e) => e.key === "Escape" && setSearch("")}
        placeholder={placeholder}
        className="min-w-0 flex-1 bg-transparent text-sm outline-none placeholder:text-ink-3"
        aria-label="Search bookmarks"
      />
      {search && (
        <button aria-label="Clear search" onClick={() => setSearch("")} className="text-ink-3 hover:text-ink">
          <X className="size-3.5" />
        </button>
      )}
    </div>
  );
});


function SortMenu() {
  const { sort, setSort } = useUi();
  return (
    <DM.Root>
      <DM.Trigger className="inline-flex h-9 items-center gap-1.5 rounded-lg px-2 text-[13px] font-medium text-ink-2 hover:bg-muted">
        <ArrowUpDown className="size-4" /> {SORTS.find((s) => s.value === sort)?.label}
      </DM.Trigger>
      <DM.Portal>
        <DM.Content align="end" sideOffset={4} className="z-50 min-w-48 rounded-xl border border-line bg-canvas p-1.5 shadow-lg">
          {SORTS.map((s) => (
            <DM.Item
              key={s.value}
              onSelect={() => setSort(s.value)}
              className="flex h-8 cursor-default items-center gap-2 rounded-[7px] px-2.5 text-[13px] outline-none data-[highlighted]:bg-muted"
            >
              {s.label}
              {s.value === sort && <Check className="ml-auto size-4 text-brand-600" />}
            </DM.Item>
          ))}
        </DM.Content>
      </DM.Portal>
    </DM.Root>
  );
}

function ViewToggle() {
  const { viewMode, setViewMode } = usePrefs();
  const opts: { v: ViewMode; label: string; icon: ReactNode }[] = [
    { v: "card", label: "Card view", icon: <LayoutGrid /> },
    { v: "compact", label: "Compact view", icon: <Rows3 /> },
    { v: "list", label: "List view", icon: <List /> },
  ];
  return (
    <div className="flex gap-0.5 rounded-[10px] bg-muted p-0.5" role="radiogroup" aria-label="View">
      {opts.map((o) => (
        <button
          key={o.v}
          role="radio"
          aria-checked={viewMode === o.v}
          aria-label={o.label}
          title={o.label}
          onClick={() => setViewMode(o.v)}
          className={clsx("grid h-8 w-9 place-items-center rounded-lg [&_svg]:size-4", viewMode === o.v ? "bg-canvas text-ink shadow-sm" : "text-ink-3 hover:text-ink")}
        >
          {o.icon}
        </button>
      ))}
    </div>
  );
}

function MoveMenu({ ids, categories }: { ids: string[]; categories: Category[] }) {
  const a = useBookmarkActions();
  return (
    <DM.Root>
      <DM.Trigger className="inline-flex h-8 items-center gap-1.5 rounded-lg px-2.5 text-[13px] font-medium text-ink-2 hover:bg-canvas">
        <FolderInput className="size-4" /> Move to <ChevronDown className="size-3.5" />
      </DM.Trigger>
      <DM.Portal>
        <DM.Content sideOffset={4} className="z-50 max-h-80 min-w-52 overflow-y-auto rounded-xl border border-line bg-canvas p-1.5 shadow-lg">
          <DM.Item onSelect={() => a.move(ids, null)} className="flex h-8 cursor-default items-center gap-2.5 rounded-[7px] px-2.5 text-[13px] outline-none data-[highlighted]:bg-muted">
            <span className="size-2.5 rounded-[3px] bg-ink-3" /> Uncategorized
          </DM.Item>
          {categories.map((c) => (
            <DM.Item
              key={c.id}
              onSelect={() => a.move(ids, c.id)}
              className={clsx("flex h-8 cursor-default items-center gap-2.5 rounded-[7px] px-2.5 text-[13px] outline-none data-[highlighted]:bg-muted", c.parentId && "pl-7")}
            >
              <span className="size-2.5 rounded-[3px]" style={{ background: c.color }} /> {c.name}
            </DM.Item>
          ))}
        </DM.Content>
      </DM.Portal>
    </DM.Root>
  );
}

function BulkBar({ ids, inTrash, categories }: { ids: string[]; inTrash: boolean; categories: Category[] }) {
  const a = useBookmarkActions();
  const clear = useUi((s) => s.clearSelection);
  const aiOn = !!useAiSettings().data?.config.enabled;
  const categorize = useCategorize();
  const btn = "inline-flex h-8 items-center gap-1.5 rounded-lg px-2.5 text-[13px] font-medium text-ink-2 hover:bg-canvas [&_svg]:size-4";
  return (
    <div className="flex h-11 items-center gap-1 rounded-xl bg-muted pr-2 pl-3.5">
      <span className="flex-1 text-[13px] font-medium">{ids.length} selected</span>
      {inTrash ? (
        <>
          <button className={btn} onClick={() => a.restore(ids)}>
            Restore
          </button>
          <button className={clsx(btn, "text-danger")} onClick={() => a.purge(ids)}>
            <Trash2 /> Delete permanently
          </button>
        </>
      ) : (
        <>
          <MoveMenu ids={ids} categories={categories} />
          {aiOn && (
            <button className={btn} onClick={() => categorize.mutate(ids)} disabled={categorize.isPending}>
              <Sparkles /> Organise with AI
            </button>
          )}
          <button className={btn} onClick={() => a.setFavourite(ids, true)}>
            <Star /> Favourite
          </button>
          <button className={btn} onClick={() => a.setFavourite(ids, false)}>
            Unfavourite
          </button>
          <button className={clsx(btn, "text-danger")} onClick={() => a.trash(ids)}>
            <Trash2 /> Delete
          </button>
        </>
      )}
      <button aria-label="Clear selection" className="grid size-8 place-items-center rounded-lg text-ink-3 hover:bg-canvas" onClick={clear}>
        <X className="size-4" />
      </button>
    </div>
  );
}

function groupByDay(items: Bookmark[]) {
  const now = new Date();
  const start = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();
  const groups: { label: string; items: Bookmark[] }[] = [];
  for (const b of items) {
    const t = new Date(b.createdAt).getTime();
    const label = t >= start ? "Today" : t >= start - 86_400_000 ? "Yesterday" : t >= start - 6 * 86_400_000 ? "Earlier this week" : "Earlier";
    const g = groups.find((x) => x.label === label);
    if (g) g.items.push(b);
    else groups.push({ label, items: [b] });
  }
  return groups;
}

export function BookmarkCollection({ items, categories, mode }: { items: Bookmark[]; categories: Category[]; mode: ViewMode }) {
  const selection = useUi((s) => s.selection);
  const byId = useMemo(() => new Map(categories.map((c) => [c.id, c])), [categories]);
  const props = (b: Bookmark) => ({ bookmark: b, category: b.categoryId ? byId.get(b.categoryId) : undefined, selected: selection.has(b.id) });
  if (mode === "list") {
    return (
      <div className="overflow-hidden rounded-[14px] border border-line">
        {items.map((b) => (
          <BookmarkRow key={b.id} {...props(b)} />
        ))}
      </div>
    );
  }
  if (mode === "compact") {
    return (
      <div className="grid grid-cols-[repeat(auto-fill,minmax(320px,1fr))] gap-2.5">
        {items.map((b) => (
          <BookmarkCompact key={b.id} {...props(b)} />
        ))}
      </div>
    );
  }
  return (
    <div className="grid grid-cols-[repeat(auto-fill,minmax(480px,1fr))] gap-5">
      {items.map((b) => (
        <BookmarkCard key={b.id} {...props(b)} />
      ))}
    </div>
  );
}

export function Library({ view, categoryId, searchRef }: { view: LibraryView; categoryId?: string; searchRef: React.Ref<HTMLInputElement> }) {
  const { search, sort, selection, navigate, openDialog } = useUi();
  const viewMode = usePrefs((s) => s.viewMode);
  const { data: categories = [] } = useCategories();
  const { data: counts } = useCounts();
  const category = categories.find((c) => c.id === categoryId);
  const subcats = categories.filter((c) => c.parentId === categoryId);
  const [filters, setFilters] = useState<FilterState>(NO_FILTERS);
  const { data: items = EMPTY, isLoading } = useBookmarks({ view, categoryId: categoryId ?? null, search, sort, ...filters });
  const filtering = activeFilterCount(filters) > 0;
  const ids = [...selection];
  const setVisible = useUi((s) => s.setVisible);
  useEffect(() => setVisible(items), [items, setVisible]);

  const meta = view === "category" ? null : META[view];
  const title = view === "category" ? (category?.name ?? "Category") : meta!.title;
  const subtitle =
    view === "category"
      ? [category?.description, plural(items.length, "bookmark"), subcats.length ? plural(subcats.length, "subcategory", "subcategories") : null]
          .filter(Boolean)
          .join(" · ")
      : meta!.sub(view === "trash" ? (counts?.trash ?? items.length) : items.length);

  const crumbs =
    view === "category" ? (
      <>
        <Folder />
        <span className="text-ink-3">Categories</span>
        <ChevronRight className="!size-3.5" />
        <span className="font-medium">{title}</span>
      </>
    ) : (
      <>
        {meta!.icon}
        <span className="font-medium">{title}</span>
      </>
    );

  return (
    <div className="flex h-full min-w-0 flex-1 flex-col">
      <TopBar crumbs={crumbs} />
      <div className="flex min-h-0 flex-1 flex-col gap-5 overflow-y-auto px-8 pt-7 pb-10">
        <div className="flex items-center gap-4">
          {view === "category" ? (
            <div className="grid size-12 place-items-center rounded-xl [&_svg]:size-[22px]" style={{ background: tint(category?.color ?? "#64748B", 0.12), color: category?.color }}>
              <Folder />
            </div>
          ) : (
            <div className={clsx("grid size-12 place-items-center rounded-xl [&_svg]:size-[22px]", meta!.box)}>{meta!.icon}</div>
          )}
          <div className="min-w-0 flex-1">
            <h1 className="truncate text-xl font-semibold tracking-tight">{title}</h1>
            <p className="truncate text-[13px] text-ink-2">{subtitle}</p>
          </div>
          {view === "category" && category && (
            <>
              <Button icon={<Pencil />} onClick={() => openDialog({ kind: "category", category })}>
                Edit category
              </Button>
              <Button variant="ghost" icon={<Trash2 />} onClick={() => openDialog({ kind: "delete-category", category })}>
                Delete
              </Button>
            </>
          )}
          {view === "security" ? (
            <SecurityHeaderAction />
          ) : view === "trash" ? (
            <Button
              className="border-red-200 text-danger"
              icon={<Trash2 />}
              disabled={!counts?.trash}
              onClick={() => openDialog({ kind: "empty-trash" })}
            >
              Empty Trash
            </Button>
          ) : (
            <Button variant="primary" icon={<Plus />} onClick={() => openDialog({ kind: "add-bookmark" })}>
              Add Bookmark
            </Button>
          )}
        </div>

        {view === "category" && subcats.length > 0 && (
          <div className="flex gap-6 border-b border-line">
            <span className="border-b-2 border-brand-600 pt-1 pb-3 text-sm font-medium text-brand-700">All</span>
            {subcats.map((s) => (
              <button
                key={s.id}
                onClick={() => navigate({ page: "library", view: "category", categoryId: s.id })}
                className="flex items-center gap-1.5 pt-1 pb-3 text-sm font-medium text-ink-2 hover:text-ink"
              >
                {s.name} <span className="text-xs text-ink-3">{s.bookmarkCount}</span>
              </button>
            ))}
          </div>
        )}

        {view === "security" && <SecuritySummary />}
        {view === "uncategorized" && (items.length > 0 || isLoading) && <AiReviewPanel />}

        {view === "trash" && items.length > 0 && (
          <div className="flex items-center gap-2.5 rounded-xl border border-amber-200 bg-warning-bg px-3.5 py-3 text-[13px]">
            <ShieldAlert className="size-4 text-warning" />
            Emptying Trash permanently deletes these bookmarks from your local database.
          </div>
        )}

        <div className="flex flex-wrap items-center gap-2">
          <SearchBox ref={searchRef} placeholder={`Search ${view === "category" ? title : title.toLowerCase()}…`} />
          <FilterBar view={view} value={filters} onChange={setFilters} />
          <div className="flex flex-1 justify-end gap-2">
            <SortMenu />
            {view !== "security" && <ViewToggle />}
          </div>
        </div>

        {ids.length > 0 && <BulkBar ids={ids} inTrash={view === "trash"} categories={categories} />}

        {isLoading ? null : items.length === 0 ? (
          <EmptyForView view={view} searching={!!search.trim() || filtering} title={title} onAdd={() => openDialog({ kind: "add-bookmark" })} onClear={() => setFilters(NO_FILTERS)} />
        ) : view === "security" ? (
          <SecurityAlertList items={items} />
        ) : view === "recent" && sort === "recently_added" ? (
          groupByDay(items).map((g) => (
            <section key={g.label} className="flex flex-col gap-2.5">
              <h2 className="flex items-center gap-2 text-[13px] font-medium text-ink-2">
                {g.label} <span className="rounded-md bg-muted px-1.5 text-xs text-ink-3">{g.items.length}</span>
              </h2>
              <BookmarkCollection items={g.items} categories={categories} mode={viewMode} />
            </section>
          ))
        ) : (
          <BookmarkCollection items={items} categories={categories} mode={viewMode} />
        )}
        {view === "security" && <TrustedSites />}
      </div>
    </div>
  );
}

function EmptyForView({ view, searching, title, onAdd, onClear }: { view: LibraryView; searching: boolean; title: string; onAdd: () => void; onClear: () => void }) {
  const setSearch = useUi((s) => s.setSearch);
  const navigate = useUi((s) => s.navigate);
  if (searching)
    return (
      <EmptyState
        icon={<Search />}
        title="No results"
        body="Check the spelling, use fewer words, or remove a filter. Search covers titles, URLs, descriptions, categories and tags."
        actions={
          <Button
            onClick={() => {
              setSearch("");
              onClear();
            }}
          >
            Clear search and filters
          </Button>
        }
      />
    );
  switch (view) {
    case "favourites":
      return (
        <EmptyState
          icon={<Star />}
          tone="warning"
          title="No favourites yet"
          body="Click the star on any bookmark, or press Ctrl D, to pin the links you use most."
          actions={<Button variant="primary" icon={<LayoutGrid />} onClick={() => navigate({ page: "library", view: "all" })}>Browse all bookmarks</Button>}
        />
      );
    case "uncategorized":
      return <EmptyState icon={<Inbox />} tone="success" title="Everything is sorted" body="No uncategorized bookmarks. New quick-saves land here until they get a category." />;
    case "trash":
      return <EmptyState icon={<Trash2 />} title="Trash is empty" body="Deleted bookmarks stay here until you restore them or empty Trash." />;
    case "security":
      return <EmptyState icon={<ShieldCheck />} tone="success" title="No security alerts" body="Local checks found nothing concerning. External scanners arrive in a later update." />;
    case "category":
      return (
        <EmptyState
          icon={<Folder />}
          title={`Nothing in ${title} yet`}
          body="Add a bookmark here, or move bookmarks in from any list using right-click › Move to category."
          actions={<Button variant="primary" onClick={onAdd}>Add bookmark</Button>}
        />
      );
    default:
      return (
        <EmptyState
          icon={<LayoutGrid />}
          tone="brand"
          title="Your library is empty"
          body="Paste your first link, or bring in your bookmarks from Chrome, Edge or an HTML file."
          actions={<Button variant="primary" icon={<Plus />} onClick={onAdd}>Add a link</Button>}
        />
      );
  }
}
