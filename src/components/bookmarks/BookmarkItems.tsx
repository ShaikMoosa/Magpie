import { Link2, MoreHorizontal, RotateCcw, Star, Trash2 } from "lucide-react";
import clsx from "clsx";
import type { KeyboardEvent, MouseEvent } from "react";
import type { Bookmark, Category } from "../../lib/types";
import { UNCATEGORIZED_COLOR } from "../../lib/colors";
import { displayUrl, formatDate } from "../../lib/format";
import { useUi } from "../../store/ui";
import { CategoryPill, SecurityBadge } from "../ui/Badges";
import { IconButton } from "../ui/Button";
import { BookmarkContextMenu, BookmarkDropdown } from "./BookmarkMenu";
import { useBookmarkActions } from "./useBookmarkActions";
import { Favicon, Thumbnail } from "./visuals";

interface ItemProps {
  bookmark: Bookmark;
  category: Category | undefined;
  selected: boolean;
}

function useItemHandlers(bookmark: Bookmark) {
  const a = useBookmarkActions();
  const { toggleSelect, navigate } = useUi();
  const onClick = (e: MouseEvent) => {
    if (e.ctrlKey || e.shiftKey || e.metaKey) toggleSelect(bookmark.id, true);
    else if (!bookmark.deletedAt) a.open(bookmark);
  };
  const onKeyDown = (e: KeyboardEvent) => {
    if (e.key === "Enter" && !bookmark.deletedAt) a.open(bookmark);
    if (e.key === " ") {
      e.preventDefault();
      toggleSelect(bookmark.id, true);
    }
  };
  const openCategory = bookmark.categoryId ? () => navigate({ page: "library", view: "category", categoryId: bookmark.categoryId! }) : undefined;
  return { a, onClick, onKeyDown, openCategory, toggleSelect };
}

function StarButton({ bookmark, size = 18 }: { bookmark: Bookmark; size?: number }) {
  const a = useBookmarkActions();
  if (bookmark.deletedAt) return null;
  return (
    <button
      type="button"
      aria-label={bookmark.isFavourite ? "Remove from Favourites" : "Add to Favourites"}
      aria-pressed={bookmark.isFavourite}
      onClick={(e) => {
        e.stopPropagation();
        a.toggleFavourite(bookmark);
      }}
      className="grid size-7 place-items-center rounded-md text-ink-3 hover:bg-muted"
    >
      <Star style={{ width: size, height: size }} className={clsx(bookmark.isFavourite && "fill-warning text-warning")} />
    </button>
  );
}

function MoreButton({ bookmark }: { bookmark: Bookmark }) {
  return (
    <BookmarkDropdown
      bookmark={bookmark}
      trigger={
        <IconButton label="More actions" className="size-7" onClick={(e) => e.stopPropagation()}>
          <MoreHorizontal />
        </IconButton>
      }
    />
  );
}

function Checkbox({ checked, onToggle, className }: { checked: boolean; onToggle: () => void; className?: string }) {
  return (
    <button
      type="button"
      role="checkbox"
      aria-checked={checked}
      aria-label="Select"
      onClick={(e) => {
        e.stopPropagation();
        onToggle();
      }}
      className={clsx(
        "grid size-[18px] shrink-0 place-items-center rounded-[5px] border transition",
        checked ? "border-brand-600 bg-brand-600 text-white" : "border-line-strong bg-canvas",
        className,
      )}
    >
      {checked && (
        <svg viewBox="0 0 24 24" className="size-3" fill="none" stroke="currentColor" strokeWidth="3">
          <path d="M20 6 9 17l-5-5" />
        </svg>
      )}
    </button>
  );
}

function TrashActions({ bookmark }: { bookmark: Bookmark }) {
  const a = useBookmarkActions();
  return (
    <div className="flex items-center gap-1.5">
      <button
        className="inline-flex h-7 items-center gap-1.5 rounded-lg border border-line-strong px-2.5 text-[13px] font-medium hover:bg-subtle"
        onClick={(e) => {
          e.stopPropagation();
          a.restore([bookmark.id]);
        }}
      >
        <RotateCcw className="size-3.5" /> Restore
      </button>
      <IconButton
        label="Delete permanently"
        className="size-7 border border-line-strong text-danger hover:text-danger"
        onClick={(e) => {
          e.stopPropagation();
          a.purge([bookmark.id]);
        }}
      >
        <Trash2 className="!size-3.5" />
      </IconButton>
    </div>
  );
}

export function BookmarkCard({ bookmark, category, selected }: ItemProps) {
  const { onClick, onKeyDown, openCategory, toggleSelect } = useItemHandlers(bookmark);
  const risky = bookmark.securityStatus === "suspicious" || bookmark.securityStatus === "dangerous";
  return (
    <BookmarkContextMenu bookmark={bookmark}>
      <div
        role="button"
        tabIndex={0}
        onClick={onClick}
        onKeyDown={onKeyDown}
        aria-selected={selected}
        className={clsx(
          "group flex gap-4 rounded-[14px] border p-3 pr-3.5 text-left shadow-sm transition hover:border-line-strong hover:shadow-md",
          selected ? "border-brand-500 bg-brand-50 ring-1 ring-brand-500" : risky ? "border-warning/50 bg-canvas" : "border-line bg-canvas",
        )}
      >
        <div className="relative">
          <Thumbnail color={category?.color ?? UNCATEGORIZED_COLOR} className="h-28 w-[156px]" />
          <Checkbox
            checked={selected}
            onToggle={() => toggleSelect(bookmark.id, true)}
            className={clsx("absolute top-2 left-2", !selected && "opacity-0 group-hover:opacity-100 focus:opacity-100")}
          />
        </div>
        <div className="flex min-w-0 flex-1 flex-col gap-1.5">
          <div className="flex items-center gap-2">
            <Favicon domain={bookmark.domain} size={18} />
            <h3 className="min-w-0 flex-1 truncate text-[15px] font-semibold tracking-tight">{bookmark.title}</h3>
            <StarButton bookmark={bookmark} />
            <MoreButton bookmark={bookmark} />
          </div>
          <p className="line-clamp-2 min-h-9 text-[13px] leading-[18px] text-ink-2">
            {bookmark.description ?? <span className="text-ink-3">No description</span>}
          </p>
          <p className="flex items-center gap-1.5 truncate text-xs text-ink-3">
            <Link2 className="size-3 shrink-0" />
            <span className="truncate">{displayUrl(bookmark.url)}</span>
          </p>
          <div className="mt-auto flex items-center gap-1.5 pt-1">
            {bookmark.deletedAt ? (
              <>
                <span className="text-xs text-ink-3">Deleted {formatDate(bookmark.deletedAt)}</span>
                <span className="ml-auto">
                  <TrashActions bookmark={bookmark} />
                </span>
              </>
            ) : (
              <>
                <CategoryPill category={category} onClick={openCategory} />
                <SecurityBadge status={bookmark.securityStatus} />
              </>
            )}
          </div>
        </div>
      </div>
    </BookmarkContextMenu>
  );
}

export function BookmarkRow({ bookmark, category, selected }: ItemProps) {
  const { onClick, onKeyDown, openCategory, toggleSelect } = useItemHandlers(bookmark);
  return (
    <BookmarkContextMenu bookmark={bookmark}>
      <div
        role="row"
        tabIndex={0}
        onClick={onClick}
        onKeyDown={onKeyDown}
        aria-selected={selected}
        className={clsx(
          "flex h-[52px] items-center gap-3.5 border-b border-line px-4 last:border-b-0",
          selected ? "bg-brand-50" : "bg-canvas hover:bg-subtle",
        )}
      >
        <Checkbox checked={selected} onToggle={() => toggleSelect(bookmark.id, true)} />
        <Favicon domain={bookmark.domain} />
        <div className="min-w-0 flex-1">
          <p className="truncate text-sm font-medium">{bookmark.title}</p>
          <p className="truncate text-xs text-ink-3">{bookmark.domain}</p>
        </div>
        <div className="w-[150px]">
          <CategoryPill category={category} onClick={openCategory} />
        </div>
        <div className="w-[130px]">
          <SecurityBadge status={bookmark.securityStatus} />
        </div>
        <span className="w-[92px] text-xs text-ink-3">{formatDate(bookmark.deletedAt ?? bookmark.createdAt)}</span>
        {bookmark.deletedAt ? (
          <TrashActions bookmark={bookmark} />
        ) : (
          <>
            <StarButton bookmark={bookmark} size={16} />
            <MoreButton bookmark={bookmark} />
          </>
        )}
      </div>
    </BookmarkContextMenu>
  );
}

export function BookmarkCompact({ bookmark, category, selected }: ItemProps) {
  const { onClick, onKeyDown } = useItemHandlers(bookmark);
  return (
    <BookmarkContextMenu bookmark={bookmark}>
      <div
        role="button"
        tabIndex={0}
        onClick={onClick}
        onKeyDown={onKeyDown}
        aria-selected={selected}
        className={clsx(
          "flex h-11 items-center gap-2.5 rounded-[10px] border px-3 transition",
          selected ? "border-brand-500 bg-brand-50 ring-1 ring-brand-500" : "border-line bg-canvas hover:border-line-strong",
        )}
      >
        <Favicon domain={bookmark.domain} />
        <div className="min-w-0 flex-1 leading-tight">
          <p className="truncate text-[13px] font-medium">{bookmark.title}</p>
          <p className="truncate text-xs text-ink-3">{bookmark.domain}</p>
        </div>
        <CategoryPill category={category} />
        <StarButton bookmark={bookmark} size={14} />
      </div>
    </BookmarkContextMenu>
  );
}
