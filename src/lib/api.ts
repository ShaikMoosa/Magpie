import { invoke } from "@tauri-apps/api/core";
import type {
  ScanProvider,
  SecurityConfig,
  SecuritySettings,
  BackupConfig,
  BackupInfo,
  BackupSummary,
  ExportFormat,
  StorageInfo,
  AiConfig,
  AiProvider,
  AiSettings,
  AiSuggestion,
  StoredSuggestion,
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
  Facets,
  RescanSummary,
  SecurityOverride,
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
  openPrivate: (id: string, confirmed = false) =>
    invoke<{ opened: boolean; bookmark: Bookmark }>("open_private", { id, confirmed }),
  addTags: (ids: string[], tags: string[]) => invoke<number>("add_tags", { ids, tags }),

  listCategories: () => invoke<Category[]>("list_categories"),
  createCategory: (input: CategoryInput) => invoke<Category>("create_category", { input }),
  updateCategory: (input: CategoryInput & { id: string }) => invoke<Category>("update_category", { input }),
  deleteCategory: (input: { id: string; moveTo: string | null; keepSubcategories: boolean }) =>
    invoke<void>("delete_category", { input }),
  reorderCategories: (ids: string[]) => invoke<void>("reorder_categories", { ids }),

  detectBrowsers: () => invoke<BrowserProfile[]>("detect_browsers"),
  previewImport: (source: ImportSource) => invoke<ImportPreview>("preview_import", { source }),
  startImport: (source: ImportSource, options: ImportOptions) => invoke<ImportSummary>("start_import", { source, options }),

  getFacets: () => invoke<Facets>("get_facets"),
  rescan: (ids: string[] | null = null) => invoke<RescanSummary>("rescan_bookmarks", { ids }),
  markSafe: (ids: string[]) => invoke<number>("mark_safe", { ids }),
  listOverrides: () => invoke<SecurityOverride[]>("list_overrides"),
  removeOverride: (domain: string) => invoke<void>("remove_override", { domain }),

  getAiSettings: () => invoke<AiSettings>("get_ai_settings"),
  saveAiConfig: (config: AiConfig) => invoke<AiConfig>("save_ai_config", { config }),
  setAiKey: (provider: AiProvider, key: string) => invoke<void>("set_ai_key", { provider, key }),
  removeAiKey: (provider: AiProvider) => invoke<void>("remove_ai_key", { provider }),
  listAiModels: (config: AiConfig) => invoke<string[]>("list_ai_models", { config }),
  suggestCategory: (draft: { url: string; title?: string | null; description?: string | null }) => invoke<AiSuggestion>("suggest_category", { draft }),
  listAiSuggestions: () => invoke<StoredSuggestion[]>("list_ai_suggestions"),
  acceptAiSuggestions: (ids: string[]) => invoke<number>("accept_ai_suggestions", { ids }),
  rejectAiSuggestions: (ids: string[]) => invoke<number>("reject_ai_suggestions", { ids }),
  categorizeBookmarks: (ids: string[] | null = null) => invoke<number>("categorize_bookmarks", { ids }),

  // Security scanners
  getSecuritySettings: () => invoke<SecuritySettings>("get_security_settings"),
  saveSecurityConfig: (config: SecurityConfig) => invoke<SecurityConfig>("save_security_config", { config }),
  setScanKey: (provider: ScanProvider, key: string) => invoke<void>("set_scan_key", { provider, key }),
  removeScanKey: (provider: ScanProvider) => invoke<void>("remove_scan_key", { provider }),
  scanBookmarks: (ids: string[] | null, confirmed: boolean) => invoke<number>("scan_bookmarks", { ids, confirmed }),

  // Data & Storage
  getStorageInfo: () => invoke<StorageInfo>("get_storage_info"),
  revealDataFolder: () => invoke<void>("reveal_data_folder"),
  exportLibrary: (format: ExportFormat, path: string, ids: string[] | null = null) => invoke<number>("export_library", { format, path, ids }),
  getBackupConfig: () => invoke<BackupConfig>("get_backup_config"),
  saveBackupConfig: (config: BackupConfig) => invoke<BackupConfig>("save_backup_config", { config }),
  listBackups: () => invoke<BackupInfo[]>("list_backups"),
  createBackup: () => invoke<BackupInfo>("create_backup"),
  inspectBackup: (src: { name?: string; path?: string }) => invoke<BackupSummary>("inspect_backup", { name: src.name ?? null, path: src.path ?? null }),
  restoreBackup: (src: { name?: string; path?: string }) => invoke<BackupSummary>("restore_backup", { name: src.name ?? null, path: src.path ?? null }),
  clearPreviewCache: () => invoke<number>("clear_preview_cache"),
  resetAppData: () => invoke<void>("reset_app_data"),

  previewsDir: () => invoke<string>("previews_dir"),
  fetchMetadata: (url: string) => invoke<PageMeta>("fetch_metadata", { url }),
  refreshMetadata: (ids: string[] | null = null) => invoke<void>("refresh_metadata", { ids }),
};
