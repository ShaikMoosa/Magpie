import * as CM from "@radix-ui/react-context-menu";
import * as DM from "@radix-ui/react-dropdown-menu";
import { Check, ChevronRight, Copy, ExternalLink, Folder, Pencil, RotateCcw, Star, Trash2 } from "lucide-react";
import clsx from "clsx";
import type { ReactNode } from "react";
import type { Bookmark } from "../../lib/types";
import { useBookmarkActions } from "./useBookmarkActions";
import { useCategories } from "../../hooks/queries";

type Kit = typeof CM | typeof DM;

const itemCls =
  "flex h-8 cursor-default items-center gap-2.5 rounded-[7px] px-2.5 text-[13px] outline-none select-none data-[highlighted]:bg-muted [&_svg]:size-4 [&_svg]:text-ink-2";
const contentCls = "z-50 min-w-[260px] rounded-xl border border-line bg-canvas p-1.5 shadow-lg";

function Items({ kit, bookmark }: { kit: Kit; bookmark: Bookmark }) {
  const a = useBookmarkActions();
  const { data: categories = [] } = useCategories();
  const K = kit as typeof DM;
  if (bookmark.deletedAt) {
    return (
      <>
        <K.Item className={itemCls} onSelect={() => a.restore([bookmark.id])}>
          <RotateCcw /> Restore
        </K.Item>
        <K.Item className={clsx(itemCls, "text-danger [&_svg]:!text-danger")} onSelect={() => a.purge([bookmark.id])}>
          <Trash2 /> Delete permanently
        </K.Item>
      </>
    );
  }
  const shortcut = (s: string) => <span className="ml-auto pl-4 text-xs text-ink-3">{s}</span>;
  return (
    <>
      <K.Item className={itemCls} onSelect={() => a.open(bookmark)}>
        <ExternalLink /> Open {shortcut("Enter")}
      </K.Item>
      <K.Item className={itemCls} onSelect={() => a.copyUrl(bookmark)}>
        <Copy /> Copy URL {shortcut("Ctrl Shift C")}
      </K.Item>
      <K.Separator className="my-1 h-px bg-line" />
      <K.Item className={itemCls} onSelect={() => a.edit(bookmark)}>
        <Pencil /> Edit… {shortcut("F2")}
      </K.Item>
      <K.Item className={itemCls} onSelect={() => a.toggleFavourite(bookmark)}>
        <Star className={clsx(bookmark.isFavourite && "fill-warning !text-warning")} />
        {bookmark.isFavourite ? "Remove from Favourites" : "Add to Favourites"} {shortcut("Ctrl D")}
      </K.Item>
      <K.Sub>
        <K.SubTrigger className={clsx(itemCls, "data-[state=open]:bg-muted")}>
          <Folder /> Move to category <ChevronRight className="ml-auto" />
        </K.SubTrigger>
        <K.Portal>
          <K.SubContent className={clsx(contentCls, "min-w-[220px]")} sideOffset={6}>
            <K.Item className={itemCls} onSelect={() => a.move([bookmark.id], null)}>
              <span className="size-2.5 rounded-[3px] bg-ink-3" /> Uncategorized
              {!bookmark.categoryId && <Check className="ml-auto !text-brand-600" />}
            </K.Item>
            {categories.map((c) => (
              <K.Item key={c.id} className={clsx(itemCls, c.parentId && "pl-7")} onSelect={() => a.move([bookmark.id], c.id)}>
                <span className="size-2.5 shrink-0 rounded-[3px]" style={{ background: c.color }} />
                <span className="truncate">{c.name}</span>
                {bookmark.categoryId === c.id && <Check className="ml-auto !text-brand-600" />}
              </K.Item>
            ))}
          </K.SubContent>
        </K.Portal>
      </K.Sub>
      <K.Separator className="my-1 h-px bg-line" />
      <K.Item className={clsx(itemCls, "text-danger [&_svg]:!text-danger")} onSelect={() => a.trash([bookmark.id])}>
        <Trash2 /> Move to Trash {shortcut("Delete")}
      </K.Item>
    </>
  );
}

export function BookmarkContextMenu({ bookmark, children }: { bookmark: Bookmark; children: ReactNode }) {
  return (
    <CM.Root>
      <CM.Trigger asChild>{children}</CM.Trigger>
      <CM.Portal>
        <CM.Content className={contentCls}>
          <Items kit={CM} bookmark={bookmark} />
        </CM.Content>
      </CM.Portal>
    </CM.Root>
  );
}

export function BookmarkDropdown({ bookmark, trigger }: { bookmark: Bookmark; trigger: ReactNode }) {
  return (
    <DM.Root>
      <DM.Trigger asChild>{trigger}</DM.Trigger>
      <DM.Portal>
        <DM.Content className={contentCls} align="end" sideOffset={4} onClick={(e) => e.stopPropagation()}>
          <Items kit={DM} bookmark={bookmark} />
        </DM.Content>
      </DM.Portal>
    </DM.Root>
  );
}
