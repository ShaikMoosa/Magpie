import { save as saveFile } from "@tauri-apps/plugin-dialog";
import { useMoveBookmarks, useOpenBookmark, useOpenPrivate, usePurge, useRescan, useRestore, useSetFavourite, useTrash } from "../../hooks/queries";
import { useExternalScan } from "../library/ExternalScan";
import type { Bookmark, ExportFormat } from "../../lib/types";
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
  const priv = useOpenPrivate();
  const rescan = useRescan();
  const scan = useExternalScan();

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
    openPrivate: (bookmark: Bookmark) =>
      priv.mutate(
        { bookmark },
        {
          onSuccess: (r) => {
            if (!r.opened) openDialog({ kind: "open-warning", bookmark: r.bookmark, private: true });
          },
        },
      ),
    openPrivateConfirmed: (bookmark: Bookmark) => priv.mutate({ bookmark, confirmed: true }),
    /** Providers when set up (asks first if required), otherwise local checks. */
    scan: (ids: string[]) => (scan.available ? scan.start(ids) : rescan.mutate(ids)),
    addTags: (ids: string[]) => openDialog({ kind: "add-tags", ids }),
    exportSelection: async (ids: string[], format: ExportFormat) => {
      try {
        const stamp = new Date().toISOString().slice(0, 10);
        const path = await saveFile({ defaultPath: `magpie-selection-${stamp}.${format}`, filters: [{ name: format.toUpperCase(), extensions: [format] }] });
        if (!path) return;
        const n = await api.exportLibrary(format, path, ids);
        toast({ type: "success", title: `Exported ${n} bookmark${n === 1 ? "" : "s"}`, body: path });
      } catch (e) {
        toast({ type: "error", title: "Couldn't export", body: errorMessage(e) });
      }
    },
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
