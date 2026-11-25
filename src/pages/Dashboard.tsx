import { ChevronRight, FileText, Globe, LayoutGrid, Link2, Plus } from "lucide-react";
import clsx from "clsx";
import { useMemo, useState } from "react";
import logo from "../assets/logo.png";
import { FolderArt } from "../components/bookmarks/visuals";
import { TopBar } from "../components/layout/TopBar";
import { Button } from "../components/ui/Button";
import { EmptyState } from "../components/ui/EmptyState";
import { useBookmarks, useCategories, useCounts } from "../hooks/queries";
import { domainColor, domainLetter } from "../lib/colors";
import { plural } from "../lib/format";
import type { Bookmark, Category } from "../lib/types";
import { usePrefs, useUi } from "../store/ui";
import { BookmarkCollection } from "./Library";

function QuickAdd() {
  const openDialog = useUi((s) => s.openDialog);
  const [url, setUrl] = useState("");
  const submit = () => {
    openDialog({ kind: "add-bookmark", url: url.trim() || undefined });
    setUrl("");
  };
  return (
    <div className="w-full max-w-[680px] overflow-hidden rounded-2xl bg-muted">
      <form
        className="flex flex-col gap-4 rounded-2xl border border-line-strong bg-canvas p-4 shadow-md"
        onSubmit={(e) => {
          e.preventDefault();
          submit();
        }}
      >
        <label className="flex items-center gap-2.5">
          <Link2 className="size-[18px] text-ink-3" />
          <input
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            placeholder="Paste a URL, e.g. https://dribbble.com/shots/…"
            className="min-w-0 flex-1 bg-transparent text-[15px] outline-none placeholder:text-ink-3"
            aria-label="Website address"
          />
        </label>
        <div className="flex justify-end">
          <button type="submit" aria-label="Add bookmark" className="grid size-9 place-items-center rounded-full bg-brand-600 text-white hover:bg-brand-700">
            <Plus className="size-[18px]" />
          </button>
        </div>
      </form>
      <div className="flex gap-2 px-3 py-2.5">
        {([
          { icon: <FileText />, label: "HTML file", source: "html" },
          { icon: <Globe />, label: "Import from Chrome", source: "chrome" },
          { icon: <Globe />, label: "Import from Edge", source: "edge" },
        ] as const).map((c) => (
          <button
            key={c.label}
            onClick={() => openDialog({ kind: "import", source: c.source })}
            className="inline-flex h-7 items-center gap-1.5 rounded-lg border border-line bg-subtle px-2.5 text-xs font-medium text-ink-2 hover:text-ink [&_svg]:size-3.5"
          >
            {c.icon}
            {c.label}
          </button>
        ))}
      </div>
    </div>
  );
}

function FolderTile({ category, total, letters, hovered }: { category: Category; total: number; letters: { letter: string; color: string }[]; hovered?: boolean }) {
  const navigate = useUi((s) => s.navigate);
  return (
    <button
      onClick={() => navigate({ page: "library", view: "category", categoryId: category.id })}
      className={clsx("group flex w-[172px] flex-col items-center gap-1.5 rounded-[20px] px-2 py-3.5 transition-colors hover:bg-muted", hovered && "bg-muted")}
    >
      <div className="transition-transform duration-200 group-hover:-translate-y-1">
        <FolderArt color={category.color} letters={letters} count={total} />
      </div>
      <span className="w-full truncate text-[15px] font-medium">{category.name}</span>
      <span className="w-full truncate text-xs text-ink-3">{category.description ?? plural(total, "bookmark")}</span>
    </button>
  );
}

export function Dashboard() {
  const { openDialog, navigate } = useUi();
  const viewMode = usePrefs((s) => s.viewMode);
  const { data: categories = [] } = useCategories();
  const { data: counts } = useCounts();
  const { data: recent = [] } = useBookmarks({ view: "all", sort: "recently_added", limit: 6 });
  const { data: all = [] } = useBookmarks({ view: "all", sort: "most_opened", limit: 300 });

  const tops = categories.filter((c) => !c.parentId).slice(0, 5);
  const tileData = useMemo(() => {
    const descendants = (id: string): string[] => categories.filter((c) => c.parentId === id).flatMap((c) => [c.id, ...descendants(c.id)]);
    return tops.map((c) => {
      const ids = new Set([c.id, ...descendants(c.id)]);
      const total = categories.filter((x) => ids.has(x.id)).reduce((n, x) => n + x.bookmarkCount, 0);
      const domains = [...new Set(all.filter((b: Bookmark) => b.categoryId && ids.has(b.categoryId)).map((b) => b.domain))].slice(0, 3);
      return { c, total, letters: domains.map((d) => ({ letter: domainLetter(d), color: domainColor(d) })) };
    });
  }, [tops, categories, all]);

  return (
    <div className="flex h-full min-w-0 flex-1 flex-col">
      <TopBar
        crumbs={
          <>
            <LayoutGrid />
            <span className="font-medium">Dashboard</span>
          </>
        }
      />
      <div className="flex min-h-0 flex-1 flex-col items-center gap-10 overflow-y-auto px-8 pt-12 pb-12">
        <section className="flex w-full max-w-[680px] flex-col gap-5">
          <div className="flex flex-col gap-2">
            <img src={logo} alt="" className="mb-1 size-10 rounded-[11px]" draggable={false} />
            <h1 className="text-[32px] leading-10 font-bold tracking-[-0.02em]">What do you want to save today?</h1>
            <p className="text-sm text-ink-2">Paste a link — Magpie checks it and helps you put it in the right place.</p>
          </div>
          <QuickAdd />
        </section>

        <section className="flex w-full flex-col gap-4">
          <div className="flex items-center gap-2">
            <h2 className="text-base font-semibold">Categories</h2>
            <span className="rounded-md bg-muted px-[7px] py-px text-xs font-medium text-ink-2">{categories.filter((c) => !c.parentId).length}</span>
            <div className="flex-1" />
            {categories.length > 0 && (
              <button onClick={() => navigate({ page: "library", view: "category", categoryId: tops[0]?.id })} className="flex items-center gap-1 text-[13px] font-medium text-ink-2 hover:text-ink">
                Manage categories <ChevronRight className="size-3.5" />
              </button>
            )}
          </div>
          <div className="flex flex-wrap gap-2">
            {tileData.map(({ c, total, letters }) => (
              <FolderTile key={c.id} category={c} total={total} letters={letters} />
            ))}
            <button onClick={() => openDialog({ kind: "category" })} className="group flex w-[172px] flex-col items-center gap-1.5 px-2 py-3.5">
              <div className="grid h-[134px] w-[156px] place-items-center rounded-2xl border border-dashed border-line-strong bg-subtle group-hover:border-brand-400">
                <span className="grid size-9 place-items-center rounded-full border border-line bg-canvas text-ink-2">
                  <Plus className="size-[18px]" />
                </span>
              </div>
              <span className="text-[15px] font-medium text-ink-2">New category</span>
              <span className="text-xs text-ink-3">Group related links</span>
            </button>
          </div>
        </section>

        <section className="flex w-full flex-col gap-4">
          <div className="flex items-center gap-2">
            <h2 className="text-base font-semibold">Recently added</h2>
            <span className="rounded-md bg-muted px-[7px] py-px text-xs font-medium text-ink-2">{counts?.recent ?? 0}</span>
            <div className="flex-1" />
            {recent.length > 0 && (
              <Button variant="ghost" size="sm" onClick={() => navigate({ page: "library", view: "recent" })}>
                View all <ChevronRight />
              </Button>
            )}
          </div>
          {recent.length === 0 ? (
            <EmptyState
              icon={<LayoutGrid />}
              tone="brand"
              title="Your library is empty"
              body="Paste your first link above, or import from Chrome, Edge or an HTML file."
              actions={
                <Button variant="primary" icon={<Plus />} onClick={() => openDialog({ kind: "add-bookmark" })}>
                  Add a link
                </Button>
              }
            />
          ) : (
            <BookmarkCollection items={recent} categories={categories} mode={viewMode === "list" ? "list" : viewMode} />
          )}
        </section>
      </div>
    </div>
  );
}
