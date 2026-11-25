import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, errorMessage } from "../lib/api";
import type { Bookmark, BookmarkQuery, CategoryInput, NewBookmark, UpdateBookmark } from "../lib/types";
import { toast } from "../store/toast";

export const keys = {
  bookmarks: (q?: BookmarkQuery) => (q ? ["bookmarks", q] : ["bookmarks"]),
  counts: ["counts"],
  categories: ["categories"],
  facets: ["facets"],
  overrides: ["overrides"],
  ai: ["ai"],
  suggestions: ["ai-suggestions"],
};

export function useAiSettings() {
  return useQuery({ queryKey: keys.ai, queryFn: api.getAiSettings });
}

export function useAiSuggestions(enabled = true) {
  return useQuery({ queryKey: keys.suggestions, queryFn: api.listAiSuggestions, enabled });
}

export function useFacets() {
  return useQuery({ queryKey: keys.facets, queryFn: api.getFacets });
}

export function useOverrides() {
  return useQuery({ queryKey: keys.overrides, queryFn: api.listOverrides });
}

export function useBookmarks(query: BookmarkQuery) {
  return useQuery({ queryKey: keys.bookmarks(query), queryFn: () => api.listBookmarks(query), placeholderData: (p) => p });
}

export function useCounts() {
  return useQuery({ queryKey: keys.counts, queryFn: api.getCounts });
}

export function useCategories() {
  return useQuery({ queryKey: keys.categories, queryFn: api.listCategories });
}

function useInvalidate() {
  const qc = useQueryClient();
  return () =>
    Promise.all([
      qc.invalidateQueries({ queryKey: keys.bookmarks() }),
      qc.invalidateQueries({ queryKey: keys.counts }),
      qc.invalidateQueries({ queryKey: keys.categories }),
      qc.invalidateQueries({ queryKey: keys.facets }),
      qc.invalidateQueries({ queryKey: keys.overrides }),
      qc.invalidateQueries({ queryKey: keys.suggestions }),
      qc.invalidateQueries({ queryKey: keys.ai }),
    ]);
}

const onError = (title: string) => (e: unknown) => toast({ type: "error", title, body: errorMessage(e) });

export function useCreateBookmark() {
  const invalidate = useInvalidate();
  return useMutation({ mutationFn: (input: NewBookmark) => api.createBookmark(input), onSuccess: invalidate });
}

export function useUpdateBookmark() {
  const invalidate = useInvalidate();
  return useMutation({ mutationFn: (input: UpdateBookmark) => api.updateBookmark(input), onSuccess: invalidate });
}

export function useSetFavourite() {
  const invalidate = useInvalidate();
  return useMutation({
    mutationFn: ({ ids, value }: { ids: string[]; value: boolean }) => api.setFavourite(ids, value),
    onSuccess: invalidate,
    onError: onError("Couldn't update favourites"),
  });
}

export function useMoveBookmarks() {
  const invalidate = useInvalidate();
  return useMutation({
    mutationFn: ({ ids, categoryId }: { ids: string[]; categoryId: string | null }) => api.moveBookmarks(ids, categoryId),
    onSuccess: invalidate,
    onError: onError("Couldn't move bookmarks"),
  });
}

export function useTrash() {
  const invalidate = useInvalidate();
  const restore = useRestore();
  return useMutation({
    mutationFn: (ids: string[]) => api.trashBookmarks(ids),
    onSuccess: async (n, ids) => {
      await invalidate();
      toast({
        type: "success",
        title: n === 1 ? "Moved to Trash" : `Moved ${n} bookmarks to Trash`,
        action: { label: "Undo", run: () => restore.mutate(ids) },
      });
    },
    onError: onError("Couldn't move to Trash"),
  });
}

export function useRestore() {
  const invalidate = useInvalidate();
  return useMutation({
    mutationFn: (ids: string[]) => api.restoreBookmarks(ids),
    onSuccess: invalidate,
    onError: onError("Couldn't restore"),
  });
}

export function usePurge() {
  const invalidate = useInvalidate();
  return useMutation({
    mutationFn: (ids: string[]) => api.purgeBookmarks(ids),
    onSuccess: invalidate,
    onError: onError("Couldn't delete"),
  });
}

export function useEmptyTrash() {
  const invalidate = useInvalidate();
  return useMutation({ mutationFn: api.emptyTrash, onSuccess: invalidate, onError: onError("Couldn't empty Trash") });
}

export function useOpenBookmark() {
  const invalidate = useInvalidate();
  return useMutation({
    mutationFn: ({ bookmark, confirmed }: { bookmark: Bookmark; confirmed?: boolean }) =>
      api.openBookmark(bookmark.id, confirmed ?? false),
    onSuccess: (r) => {
      if (r.opened) void invalidate();
    },
    onError: onError("Couldn't open the website"),
  });
}

export function useSaveCategory() {
  const invalidate = useInvalidate();
  return useMutation({
    mutationFn: (input: CategoryInput & { id?: string }) =>
      input.id ? api.updateCategory({ ...input, id: input.id }) : api.createCategory(input),
    onSuccess: invalidate,
  });
}

export function useRescan() {
  const invalidate = useInvalidate();
  return useMutation({
    mutationFn: (ids: string[] | null) => api.rescan(ids),
    onSuccess: async (r) => {
      await invalidate();
      toast({ type: r.suspicious ? "warning" : "success", title: `Checked ${r.scanned.toLocaleString()} bookmark${r.scanned === 1 ? "" : "s"}`, body: r.suspicious ? `${r.suspicious} look suspicious.` : "No warning signs found by local checks." });
    },
    onError: onError("Couldn't run the scan"),
  });
}

export function useMarkSafe() {
  const invalidate = useInvalidate();
  return useMutation({
    mutationFn: (ids: string[]) => api.markSafe(ids),
    onSuccess: async () => {
      await invalidate();
      toast({ type: "success", title: "Marked as safe", body: "Magpie won't warn about this site again. Undo it in Security Alerts." });
    },
    onError: onError("Couldn't update the site"),
  });
}

export function useRemoveOverride() {
  const invalidate = useInvalidate();
  return useMutation({ mutationFn: (domain: string) => api.removeOverride(domain), onSuccess: invalidate, onError: onError("Couldn't update the site") });
}

export function useAcceptSuggestions() {
  const invalidate = useInvalidate();
  return useMutation({
    mutationFn: (ids: string[]) => api.acceptAiSuggestions(ids),
    onSuccess: async (n) => {
      await invalidate();
      toast({ type: "success", title: n === 1 ? "Bookmark filed" : `Filed ${n} bookmarks` });
    },
    onError: onError("Couldn't apply the suggestion"),
  });
}

export function useRejectSuggestions() {
  const invalidate = useInvalidate();
  return useMutation({ mutationFn: (ids: string[]) => api.rejectAiSuggestions(ids), onSuccess: invalidate, onError: onError("Couldn't dismiss the suggestion") });
}

export function useCategorize() {
  const invalidate = useInvalidate();
  return useMutation({
    mutationFn: (ids: string[] | null) => api.categorizeBookmarks(ids),
    onSuccess: async (n) => {
      await invalidate();
      toast(n ? { type: "info", title: `AI is looking at ${n} bookmark${n === 1 ? "" : "s"}…`, body: "Suggestions appear in Uncategorized as they arrive." } : { type: "info", title: "Nothing new to organise" });
    },
    onError: onError("Couldn't start AI organising"),
  });
}

export function useDeleteCategory() {
  const invalidate = useInvalidate();
  return useMutation({
    mutationFn: api.deleteCategory,
    onSuccess: invalidate,
    onError: onError("Couldn't delete the category"),
  });
}
