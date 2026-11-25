import { invoke } from "@tauri-apps/api/core";
import type {
  AppError,
  BrowserProfile,
  ImportOptions,
  ImportPreview,
  ImportSource,
  ImportSummary,
  Bookmark,
  BookmarkQuery,
  Category,
  CategoryInput,
  Counts,
  NewBookmark,
  PageMeta,
  UpdateBookmark,
  UrlCheck,
} from "./types";

export function isAppError(e: unknown): e is AppError {
  return typeof e === "object" && e !== null && "code" in e && "message" in e;
}

export function errorMessage(e: unknown): string {
  if (isAppError(e)) return e.message;
  if (e instanceof Error) return e.message;
  return String(e);
}

export const api = {
  listBookmarks: (query: BookmarkQuery) => invoke<Bookmark[]>("list_bookmarks", { query }),
  getCounts: () => invoke<Counts>("get_counts"),
  checkUrl: (url: string, excludeId?: string) => invoke<UrlCheck>("check_url", { url, excludeId: excludeId ?? null }),
  createBookmark: (input: NewBookmark) => invoke<Bookmark>("create_bookmark", { input }),
  updateBookmark: (input: UpdateBookmark) => invoke<Bookmark>("update_bookmark", { input }),
  setFavourite: (ids: string[], value: boolean) => invoke<number>("set_favourite", { ids, value }),
  moveBookmarks: (ids: string[], categoryId: string | null) => invoke<number>("move_bookmarks", { ids, categoryId }),
  trashBookmarks: (ids: string[]) => invoke<number>("trash_bookmarks", { ids }),
  restoreBookmarks: (ids: string[]) => invoke<number>("restore_bookmarks", { ids }),
  purgeBookmarks: (ids: string[]) => invoke<number>("purge_bookmarks", { ids }),
  emptyTrash: () => invoke<number>("empty_trash"),
  openBookmark: (id: string, confirmed = false) =>
    invoke<{ opened: boolean; bookmark: Bookmark }>("open_bookmark", { id, confirmed }),

  listCategories: () => invoke<Category[]>("list_categories"),
  createCategory: (input: CategoryInput) => invoke<Category>("create_category", { input }),
  updateCategory: (input: CategoryInput & { id: string }) => invoke<Category>("update_category", { input }),
  deleteCategory: (input: { id: string; moveTo: string | null; keepSubcategories: boolean }) =>
    invoke<void>("delete_category", { input }),
  reorderCategories: (ids: string[]) => invoke<void>("reorder_categories", { ids }),

  detectBrowsers: () => invoke<BrowserProfile[]>("detect_browsers"),
  previewImport: (source: ImportSource) => invoke<ImportPreview>("preview_import", { source }),
  startImport: (source: ImportSource, options: ImportOptions) => invoke<ImportSummary>("start_import", { source, options }),

  previewsDir: () => invoke<string>("previews_dir"),
  fetchMetadata: (url: string) => invoke<PageMeta>("fetch_metadata", { url }),
  refreshMetadata: (ids: string[] | null = null) => invoke<void>("refresh_metadata", { ids }),
};
