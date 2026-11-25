import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { PanelLeft } from "lucide-react";
import { useEffect, useRef } from "react";
import { useBookmarkActions } from "./components/bookmarks/useBookmarkActions";
import { BookmarkDialog } from "./components/dialogs/BookmarkDialog";
import { CategoryDialog, DeleteCategoryDialog } from "./components/dialogs/CategoryDialogs";
import { ConfirmTrashDialog, EmptyTrashDialog, OpenWarningDialog } from "./components/dialogs/SmallDialogs";
import { ImportDialog } from "./components/dialogs/ImportDialog";
import { useCategories, useCounts } from "./hooks/queries";
import { Welcome } from "./pages/Welcome";
import { Sidebar } from "./components/layout/Sidebar";
import { IconButton } from "./components/ui/Button";
import { Toaster } from "./components/ui/Toaster";
import { Dashboard } from "./pages/Dashboard";
import { Library } from "./pages/Library";
import { Settings } from "./pages/Settings";
import { usePrefs, useUi } from "./store/ui";

const queryClient = new QueryClient({
  defaultOptions: { queries: { staleTime: 30_000, refetchOnWindowFocus: false, retry: false } },
});

function isTyping(e: KeyboardEvent) {
  const t = e.target as HTMLElement | null;
  return !!t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.tagName === "SELECT" || t.isContentEditable);
}

function Dialogs() {
  const { dialog, closeDialog } = useUi();
  if (!dialog) return null;
  switch (dialog.kind) {
    case "add-bookmark":
      return <BookmarkDialog mode="add" initialUrl={dialog.url} onClose={closeDialog} />;
    case "edit-bookmark":
      return <BookmarkDialog mode="edit" bookmark={dialog.bookmark} onClose={closeDialog} />;
    case "category":
      return <CategoryDialog category={dialog.category} parentId={dialog.parentId} onClose={closeDialog} />;
    case "delete-category":
      return <DeleteCategoryDialog category={dialog.category} onClose={closeDialog} />;
    case "open-warning":
      return <OpenWarningDialog bookmark={dialog.bookmark} onClose={closeDialog} />;
    case "confirm-delete":
      return <ConfirmTrashDialog ids={dialog.ids} onClose={closeDialog} />;
    case "empty-trash":
      return <EmptyTrashDialog onClose={closeDialog} />;
    case "import":
      return <ImportDialog initial={dialog.source} onClose={closeDialog} />;
  }
}

function useShortcuts(focusSearch: () => void) {
  const a = useBookmarkActions();
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const ui = useUi.getState();
      const ctrl = e.ctrlKey || e.metaKey;
      const key = e.key.toLowerCase();
      const selected = ui.visible.filter((b) => ui.selection.has(b.id));

      if (ctrl && (key === "k" || (key === "f" && !e.shiftKey))) {
        e.preventDefault();
        focusSearch();
        return;
      }
      if (ui.dialog) return; // Radix handles Esc/focus inside dialogs
      if (ctrl && key === "n") {
        e.preventDefault();
        ui.openDialog({ kind: "add-bookmark" });
      } else if (ctrl && key === "i") {
        e.preventDefault();
        ui.openDialog({ kind: "import" });
      } else if (ctrl && e.shiftKey && key === "f") {
        e.preventDefault();
        ui.navigate({ page: "library", view: "favourites" });
      } else if (isTyping(e)) {
        return;
      } else if (key === "escape") {
        ui.clearSelection();
      } else if (ctrl && key === "a" && ui.visible.length) {
        e.preventDefault();
        ui.setSelection(ui.visible.map((b) => b.id));
      } else if (key === "delete" && selected.length) {
        e.preventDefault();
        if (selected.every((b) => b.deletedAt)) a.purge(selected.map((b) => b.id));
        else a.trash(selected.map((b) => b.id));
      } else if (ctrl && key === "enter" && selected[0]) {
        a.open(selected[0]);
      } else if (ctrl && key === "d" && selected.length) {
        e.preventDefault();
        a.setFavourite(
          selected.map((b) => b.id),
          !selected.every((b) => b.isFavourite),
        );
      } else if (key === "f2" && selected.length === 1) {
        a.edit(selected[0]);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [a, focusSearch]);
}

function Shell() {
  const { route, navigate, sidebarCollapsed, toggleSidebar, setSort } = useUi();
  const defaultSort = usePrefs((s) => s.defaultSort);
  const searchRef = useRef<HTMLInputElement>(null);
  const welcomeDone = usePrefs((s) => s.welcomeDone);
  const { data: counts } = useCounts();
  const { data: categories } = useCategories();
  const firstRun = !welcomeDone && counts?.all === 0 && counts.trash === 0 && categories?.length === 0;

  const focusSearch = () => {
    const ui = useUi.getState();
    if (ui.route.page !== "library") navigate({ page: "library", view: "all" });
    requestAnimationFrame(() => searchRef.current?.focus());
  };
  useShortcuts(focusSearch);

  const routeKey = JSON.stringify(route);
  useEffect(() => setSort(defaultSort), [routeKey, defaultSort, setSort]);

  // Block the WebView's own context menu outside our custom menus.
  useEffect(() => {
    const block = (e: MouseEvent) => {
      if (!(e.target as HTMLElement).closest("input, textarea")) e.preventDefault();
    };
    window.addEventListener("contextmenu", block);
    return () => window.removeEventListener("contextmenu", block);
  }, []);

  return (
    <div className="flex h-full bg-canvas">
      {firstRun ? (
        <div className="min-w-0 flex-1">
          <Welcome />
        </div>
      ) : (
        <>
          {sidebarCollapsed ? (
            <div className="flex w-14 flex-col items-center border-r border-line bg-subtle pt-3">
              <IconButton label="Expand sidebar" onClick={toggleSidebar}>
                <PanelLeft />
              </IconButton>
            </div>
          ) : (
            <Sidebar onSearchFocus={focusSearch} />
          )}
          <main className="flex min-w-0 flex-1">
            {route.page === "dashboard" && <Dashboard />}
            {route.page === "library" && <Library key={routeKey} view={route.view} categoryId={route.categoryId} searchRef={searchRef} />}
            {route.page === "settings" && <Settings section={route.section} />}
          </main>
        </>
      )}
      <Dialogs />
      <Toaster />
    </div>
  );
}

export default function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <Shell />
    </QueryClientProvider>
  );
}
