import { create } from "zustand";
import { persist } from "zustand/middleware";
import type { Bookmark, Category, LibraryView, Sort, ViewMode } from "../lib/types";

export type Route = { page: "dashboard" } | { page: "library"; view: LibraryView; categoryId?: string } | { page: "settings"; section: SettingsSection };

export type SettingsSection = "general" | "ai" | "security" | "data" | "shortcuts" | "about";

export type DialogState =
  | { kind: "add-bookmark"; url?: string }
  | { kind: "edit-bookmark"; bookmark: Bookmark }
  | { kind: "category"; category?: Category; parentId?: string | null }
  | { kind: "delete-category"; category: Category }
  | { kind: "open-warning"; bookmark: Bookmark; private?: boolean }
  | { kind: "add-tags"; ids: string[] }
  | { kind: "confirm-delete"; ids: string[] }
  | { kind: "empty-trash" }
  | { kind: "import"; source?: "chrome" | "edge" | "html" }
  | null;

interface UiState {
  route: Route;
  search: string;
  sort: Sort;
  selection: Set<string>;
  dialog: DialogState;
  sidebarCollapsed: boolean;
  /** Bookmarks currently on screen (for keyboard shortcuts). */
  visible: Bookmark[];
  setVisible: (b: Bookmark[]) => void;
  navigate: (route: Route) => void;
  setSearch: (s: string) => void;
  setSort: (s: Sort) => void;
  toggleSelect: (id: string, additive: boolean) => void;
  setSelection: (ids: string[]) => void;
  clearSelection: () => void;
  openDialog: (d: DialogState) => void;
  closeDialog: () => void;
  toggleSidebar: () => void;
}

export const useUi = create<UiState>()((set) => ({
  route: { page: "dashboard" },
  search: "",
  sort: "recently_added",
  selection: new Set(),
  dialog: null,
  sidebarCollapsed: false,
  visible: [],
  setVisible: (visible) => set({ visible }),
  navigate: (route) => set({ route, selection: new Set(), search: "" }),
  setSearch: (search) => set({ search }),
  setSort: (sort) => set({ sort }),
  toggleSelect: (id, additive) =>
    set((s) => {
      const next = new Set(additive ? s.selection : []);
      if (s.selection.has(id) && additive) next.delete(id);
      else next.add(id);
      return { selection: next };
    }),
  setSelection: (ids) => set({ selection: new Set(ids) }),
  clearSelection: () => set({ selection: new Set() }),
  openDialog: (dialog) => set({ dialog }),
  closeDialog: () => set({ dialog: null }),
  toggleSidebar: () => set((s) => ({ sidebarCollapsed: !s.sidebarCollapsed })),
}));

interface Prefs {
  viewMode: ViewMode;
  defaultSort: Sort;
  confirmDelete: boolean;
  /** First-run welcome screen was dismissed or an import finished. */
  welcomeDone: boolean;
  setWelcomeDone: (v: boolean) => void;
  /** Fetch titles, descriptions and preview images from websites. */
  autoPreviews: boolean;
  setAutoPreviews: (v: boolean) => void;
  setViewMode: (v: ViewMode) => void;
  setDefaultSort: (s: Sort) => void;
  setConfirmDelete: (v: boolean) => void;
}

/** User preferences kept in local storage (no sensitive data). */
export const usePrefs = create<Prefs>()(
  persist(
    (set) => ({
      viewMode: "card",
      defaultSort: "recently_added",
      confirmDelete: true,
      welcomeDone: false,
      setWelcomeDone: (welcomeDone) => set({ welcomeDone }),
      autoPreviews: true,
      setAutoPreviews: (autoPreviews) => set({ autoPreviews }),
      setViewMode: (viewMode) => set({ viewMode }),
      setDefaultSort: (defaultSort) => set({ defaultSort }),
      setConfirmDelete: (confirmDelete) => set({ confirmDelete }),
    }),
    { name: "magpie.prefs" },
  ),
);
