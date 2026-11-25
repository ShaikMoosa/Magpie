import { useMoveBookmarks, useOpenBookmark, usePurge, useRestore, useSetFavourite, useTrash } from "../../hooks/queries";
import type { Bookmark } from "../../lib/types";
import { api, errorMessage } from "../../lib/api";
import { toast } from "../../store/toast";
import { usePrefs, useUi } from "../../store/ui";

/** Shared actions for cards, rows, menus and keyboard shortcuts. */
export function useBookmarkActions() {
  const openDialog = useUi((s) => s.openDialog);
  const confirmDelete = usePrefs((s) => s.confirmDelete);
  const open = useOpenBookmark();
  const fav = useSetFavourite();
  const moveM = useMoveBookmarks();
  const trashM = useTrash();
  const restoreM = useRestore();
  const purgeM = usePurge();

  return {
    open: (bookmark: Bookmark) =>
      open.mutate(
        { bookmark },
        {
          onSuccess: (r) => {
            if (!r.opened) openDialog({ kind: "open-warning", bookmark: r.bookmark });
          },
        },
      ),
    openConfirmed: (bookmark: Bookmark) => open.mutate({ bookmark, confirmed: true }),
    copyUrl: async (bookmark: Bookmark) => {
      await navigator.clipboard.writeText(bookmark.url);
      toast({ type: "success", title: "URL copied" });
    },
    edit: (bookmark: Bookmark) => openDialog({ kind: "edit-bookmark", bookmark }),
    toggleFavourite: (bookmark: Bookmark) => fav.mutate({ ids: [bookmark.id], value: !bookmark.isFavourite }),
    setFavourite: (ids: string[], value: boolean) => fav.mutate({ ids, value }),
    move: (ids: string[], categoryId: string | null) =>
      moveM.mutate(
        { ids, categoryId },
        { onSuccess: (n) => toast({ type: "success", title: n === 1 ? "Bookmark moved" : `Moved ${n} bookmarks` }) },
      ),
    trash: (ids: string[]) => {
      if (confirmDelete && ids.length > 1) openDialog({ kind: "confirm-delete", ids });
      else trashM.mutate(ids);
    },
    trashNow: (ids: string[]) => trashM.mutate(ids),
    restore: (ids: string[]) =>
      restoreM.mutate(ids, { onSuccess: (n) => toast({ type: "success", title: n === 1 ? "Bookmark restored" : `Restored ${n} bookmarks` }) }),
    refreshPreview: (ids: string[]) =>
      api
        .refreshMetadata(ids)
        .then(() => toast({ type: "info", title: ids.length === 1 ? "Refreshing preview…" : `Refreshing ${ids.length} previews…` }))
        .catch((e) => toast({ type: "error", title: "Couldn't refresh the preview", body: errorMessage(e) })),
    purge: (ids: string[]) =>
      purgeM.mutate(ids, { onSuccess: (n) => toast({ type: "success", title: n === 1 ? "Deleted permanently" : `Deleted ${n} bookmarks permanently` }) }),
  };
}
