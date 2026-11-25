// Mirrors magpie-core/src/models.rs (serde camelCase).

export type SecurityStatus = "dangerous" | "suspicious" | "no_threat" | "not_checked";

export interface Bookmark {
  id: string;
  url: string;
  title: string;
  description: string | null;
  domain: string;
  faviconPath: string | null;
  thumbnailPath: string | null;
  categoryId: string | null;
  isFavourite: boolean;
  sourceBrowser: string | null;
  sourceFolder: string | null;
  securityStatus: SecurityStatus;
  securityReasons: string[];
  securitySource: string | null;
  securityCheckedAt: string | null;
  tags: string[];
  createdAt: string;
  updatedAt: string;
  lastOpenedAt: string | null;
  openCount: number;
  deletedAt: string | null;
  metadataStatus: "ok" | "failed" | null;
}

export interface NewBookmark {
  url: string;
  title?: string | null;
  description?: string | null;
  categoryId?: string | null;
  isFavourite?: boolean;
  tags?: string[];
  allowDuplicate?: boolean;
  thumbnail?: string | null;
  favicon?: string | null;
}

export interface UpdateBookmark {
  id: string;
  url: string;
  title: string;
  description: string | null;
  categoryId: string | null;
  isFavourite: boolean;
  tags: string[];
}

export type LibraryView = "all" | "favourites" | "recent" | "uncategorized" | "security" | "trash" | "category";

export type Sort =
  | "recently_added"
  | "recently_modified"
  | "title_asc"
  | "title_desc"
  | "domain"
  | "recently_opened"
  | "most_opened";

export interface BookmarkQuery {
  view: LibraryView;
  categoryId?: string | null;
  search?: string | null;
  sort?: Sort;
  favouritesOnly?: boolean;
  security?: SecurityStatus | null;
  domain?: string | null;
  tags?: string[];
  source?: string | null;
  addedWithinDays?: number | null;
  limit?: number;
}

export interface Facet {
  value: string;
  count: number;
}

export interface Facets {
  domains: Facet[];
  tags: Facet[];
  sources: Facet[];
  security: Facet[];
}

export interface RescanSummary {
  scanned: number;
  suspicious: number;
}

export interface SecurityOverride {
  domain: string;
  createdAt: string;
}

export interface Category {
  id: string;
  name: string;
  description: string | null;
  color: string;
  icon: string | null;
  parentId: string | null;
  sortOrder: number;
  createdAt: string;
  updatedAt: string;
  bookmarkCount: number;
}

export interface CategoryInput {
  name: string;
  description: string | null;
  color: string;
  icon: string | null;
  parentId: string | null;
}

export interface Counts {
  all: number;
  favourites: number;
  recent: number;
  uncategorized: number;
  securityAlerts: number;
  trash: number;
}

export interface UrlCheck {
  url: string;
  normalized: string;
  domain: string;
  duplicateOf: string | null;
  securityStatus: SecurityStatus;
  securityReasons: string[];
}

export interface AppError {
  code: string;
  message: string;
  existingId: string | null;
}

export type ViewMode = "card" | "compact" | "list";

export type BrowserName = "chrome" | "edge";

export interface BrowserProfile {
  browser: BrowserName;
  dir: string;
  name: string;
  bookmarkCount: number;
}

export type ImportSource = { kind: "browser"; browser: BrowserName; profile: string } | { kind: "html"; path: string };

export interface ImportFolder {
  key: string;
  name: string;
  depth: number;
  count: number;
}

export interface ImportPreview {
  sourceLabel: string;
  total: number;
  folders: number;
  valid: number;
  invalid: number;
  duplicatesInFile: number;
  alreadyInLibrary: number;
  newCount: number;
  folderTree: ImportFolder[];
}

export interface ImportOptions {
  selectedFolders: string[];
  foldersAsCategories: boolean;
  skipDuplicates: boolean;
  destinationCategoryId: string | null;
}

export interface ImportSummary {
  total: number;
  imported: number;
  duplicatesSkipped: number;
  invalidSkipped: number;
  categoriesCreated: number;
}

export interface PageMeta {
  title: string | null;
  description: string | null;
  siteName: string | null;
  image: string | null;
  favicon: string | null;
  finalUrl: string | null;
}

export type AiProvider = "openai" | "anthropic" | "gemini" | "custom";
export type AiMode = "manual" | "suggestion" | "automatic";

export interface AiConfig {
  enabled: boolean;
  provider: AiProvider;
  model: string;
  endpoint: string;
  mode: AiMode;
  allowNewCategories: boolean;
  neverOverwriteManual: boolean;
  autoCategoriseNew: boolean;
  excludedCategoryIds: string[];
}

export interface AiUsage {
  requests: number;
  suggestions: number;
  accepted: number;
  errors: number;
  lastError: string | null;
  lastUsedAt: string | null;
}

export interface AiSettings {
  config: AiConfig;
  keys: AiProvider[];
  usage: AiUsage;
  running: boolean;
}

export interface AiSuggestion {
  categoryId: string | null;
  newCategory: string | null;
  tags: string[];
  confidence: number;
  reason: string;
  provider: string;
  model: string;
}

export interface StoredSuggestion extends Omit<AiSuggestion, "reason"> {
  bookmark: Bookmark;
  reason: string | null;
  createdAt: string;
}

export interface AiProgress {
  done: number;
  total: number;
  suggested: number;
  applied: number;
  failed: number;
  finished: boolean;
  error: string | null;
}
