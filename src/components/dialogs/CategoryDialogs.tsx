import { Book, Bookmark, Briefcase, Code2, FileText, Folder, Globe, Link2, Palette, Sparkles, Star, Tag, Trash2, type LucideIcon } from "lucide-react";
import clsx from "clsx";
import { useState } from "react";
import { useCategories, useDeleteCategory, useSaveCategory } from "../../hooks/queries";
import { errorMessage } from "../../lib/api";
import { CATEGORY_PALETTE, isHexColor, tint } from "../../lib/colors";
import type { Category } from "../../lib/types";
import { toast } from "../../store/toast";
import { useUi } from "../../store/ui";
import { FolderArt } from "../bookmarks/visuals";
import { Button } from "../ui/Button";
import { Dialog } from "../ui/Dialog";
import { Field, TextInput } from "../ui/Field";
import { CategorySelect, descendantIds } from "./CategorySelect";

export const CATEGORY_ICONS: Record<string, LucideIcon> = {
  folder: Folder,
  bookmark: Bookmark,
  star: Star,
  globe: Globe,
  sparkles: Sparkles,
  tag: Tag,
  file: FileText,
  link: Link2,
  code: Code2,
  palette: Palette,
  book: Book,
  briefcase: Briefcase,
};

export function CategoryDialog({ category, parentId, onClose }: { category?: Category; parentId?: string | null; onClose: () => void }) {
  const { data: categories = [] } = useCategories();
  const save = useSaveCategory();
  const openDialog = useUi((s) => s.openDialog);
  const [name, setName] = useState(category?.name ?? "");
  const [description, setDescription] = useState(category?.description ?? "");
  const [parent, setParent] = useState<string | null>(category?.parentId ?? parentId ?? null);
  const [color, setColor] = useState(category?.color ?? CATEGORY_PALETTE[7]);
  const [hex, setHex] = useState(category?.color ?? CATEGORY_PALETTE[7]);
  const [icon, setIcon] = useState<string>(category?.icon ?? "folder");
  const [err, setErr] = useState<string | null>(null);

  const exclude = category ? new Set([category.id, ...descendantIds(categories, category.id)]) : undefined;
  const valid = name.trim().length > 0 && isHexColor(color);

  const submit = async () => {
    setErr(null);
    try {
      await save.mutateAsync({ id: category?.id, name, description: description || null, color, icon, parentId: parent });
      toast({ type: "success", title: category ? "Category updated" : `Created “${name.trim()}”` });
      onClose();
    } catch (e) {
      setErr(errorMessage(e));
    }
  };

  return (
    <Dialog
      open
      onClose={onClose}
      title={category ? "Edit category" : "New category"}
      icon={<Folder />}
      width={780}
      footer={
        <>
          {category ? (
            <Button variant="ghost" className="!text-danger" icon={<Trash2 />} onClick={() => openDialog({ kind: "delete-category", category })}>
              Delete category
            </Button>
          ) : (
            <span className="text-xs text-ink-3">Double-click a category in the sidebar to edit it.</span>
          )}
          <div className="flex-1" />
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="primary" disabled={!valid || save.isPending} onClick={() => void submit()}>
            {category ? "Save changes" : "Create category"}
          </Button>
        </>
      }
    >
      <div className="flex">
        <form
          className="flex min-w-0 flex-1 flex-col gap-4 px-6 py-5"
          onSubmit={(e) => {
            e.preventDefault();
            if (valid) void submit();
          }}
        >
          <Field label="Name" hint={`${name.length} / 40`}>
            <TextInput autoFocus value={name} maxLength={40} onChange={(e) => setName(e.target.value)} placeholder="e.g. Photography" />
          </Field>
          <Field label="Description (optional)">
            <TextInput value={description} maxLength={120} onChange={(e) => setDescription(e.target.value)} placeholder="What goes in here?" />
          </Field>
          <Field label="Parent category">
            <CategorySelect categories={categories} value={parent} onChange={setParent} noneLabel="None — top level" exclude={exclude} />
          </Field>
          <div className="flex flex-col gap-2">
            <span className="text-xs font-medium text-ink-2">Colour</span>
            <div className="flex items-center gap-2.5">
              {CATEGORY_PALETTE.map((c) => (
                <button
                  key={c}
                  type="button"
                  aria-label={`Colour ${c}`}
                  aria-pressed={color.toUpperCase() === c}
                  onClick={() => {
                    setColor(c);
                    setHex(c);
                  }}
                  className={clsx("size-[26px] rounded-full", color.toUpperCase() === c && "ring-2 ring-white ring-offset-2")}
                  style={{ background: c, ["--tw-ring-offset-color" as string]: c }}
                />
              ))}
              <div className="flex-1" />
              <label className="flex h-8 items-center gap-1.5 rounded-lg border border-line-strong px-2">
                <span className="size-4 rounded" style={{ background: isHexColor(hex) ? hex : "transparent" }} />
                <input
                  value={hex}
                  onChange={(e) => {
                    const v = e.target.value.startsWith("#") ? e.target.value : `#${e.target.value}`;
                    setHex(v);
                    if (isHexColor(v)) setColor(v.toUpperCase());
                  }}
                  aria-label="Custom hex colour"
                  className={clsx("w-[68px] bg-transparent text-[13px] font-medium outline-none", !isHexColor(hex) && "text-danger")}
                  maxLength={7}
                />
              </label>
            </div>
          </div>
          <div className="flex flex-col gap-2">
            <span className="text-xs font-medium text-ink-2">Icon (optional)</span>
            <div className="flex flex-wrap gap-1.5">
              {Object.entries(CATEGORY_ICONS).map(([id, Icon]) => (
                <button
                  key={id}
                  type="button"
                  aria-label={`Icon ${id}`}
                  aria-pressed={icon === id}
                  onClick={() => setIcon(id)}
                  className={clsx("grid size-[34px] place-items-center rounded-lg border", icon === id ? "border-[1.5px]" : "border-line text-ink-2 hover:bg-subtle")}
                  style={icon === id ? { borderColor: color, background: tint(color, 0.1), color } : undefined}
                >
                  <Icon className="size-4" />
                </button>
              ))}
            </div>
          </div>
          {err && <p className="text-[13px] text-danger">{err}</p>}
        </form>
        <aside className="flex w-[260px] shrink-0 flex-col items-center gap-3.5 border-l border-line bg-subtle p-5">
          <span className="self-start text-xs font-medium text-ink-2">Preview</span>
          <FolderArt color={isHexColor(color) ? color : CATEGORY_PALETTE[7]} letters={[]} count={category?.bookmarkCount ?? 0} />
          <span className="text-[15px] font-medium">{name.trim() || "Untitled"}</span>
          <div className="w-full rounded-[10px] border border-line bg-canvas p-2">
            <span className="block px-1 pb-1 text-[11px] font-semibold tracking-[0.04em] text-ink-3">IN THE SIDEBAR</span>
            <div className="flex items-center gap-2.5 rounded-lg px-2 py-1.5" style={{ background: tint(color, 0.1) }}>
              <Folder className="size-4" style={{ color }} />
              <span className="flex-1 truncate text-sm font-medium">{name.trim() || "Untitled"}</span>
              <span className="text-xs text-ink-3">{category?.bookmarkCount ?? 0}</span>
            </div>
          </div>
        </aside>
      </div>
    </Dialog>
  );
}

export function DeleteCategoryDialog({ category, onClose }: { category: Category; onClose: () => void }) {
  const { data: categories = [] } = useCategories();
  const del = useDeleteCategory();
  const { navigate, route } = useUi();
  const subs = categories.filter((c) => c.parentId === category.id);
  const [mode, setMode] = useState<"uncategorized" | "other">("uncategorized");
  const [target, setTarget] = useState<string | null>(null);
  const [keepSubs, setKeepSubs] = useState(true);
  const exclude = new Set([category.id, ...(keepSubs ? [] : descendantIds(categories, category.id))]);
  const total = category.bookmarkCount + (keepSubs ? 0 : subs.reduce((n, s) => n + s.bookmarkCount, 0));

  const confirm = () =>
    del.mutate(
      { id: category.id, moveTo: mode === "other" ? target : null, keepSubcategories: keepSubs },
      {
        onSuccess: () => {
          toast({ type: "success", title: `Deleted “${category.name}”`, body: `${total} bookmark${total === 1 ? "" : "s"} moved — none were deleted.` });
          if (route.page === "library" && route.view === "category") navigate({ page: "dashboard" });
          onClose();
        },
      },
    );

  const radio = (on: boolean) => <span className={clsx("mt-0.5 size-4 shrink-0 rounded-full border bg-canvas", on ? "border-[5px] border-brand-600" : "border-line-strong")} />;

  return (
    <Dialog
      open
      onClose={onClose}
      title={`Delete “${category.name}”?`}
      icon={<Trash2 />}
      iconClass="bg-danger-bg text-danger"
      width={500}
      footer={
        <>
          <span className="flex-1 text-xs text-ink-3">You can’t undo this.</span>
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="danger" icon={<Trash2 />} disabled={(mode === "other" && !target) || del.isPending} onClick={confirm}>
            Delete category
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-3.5 px-6 py-5">
        <p className="text-[13px] text-ink-2">
          {category.name} has {category.bookmarkCount} bookmark{category.bookmarkCount === 1 ? "" : "s"}
          {subs.length > 0 && ` and ${subs.length} subcategor${subs.length === 1 ? "y" : "ies"} (${subs.map((s) => s.name).join(", ")})`}. Bookmarks are never deleted — choose where they go.
        </p>
        <button
          type="button"
          onClick={() => setMode("uncategorized")}
          className={clsx("flex gap-3 rounded-xl border p-3.5 text-left", mode === "uncategorized" ? "border-[1.5px] border-brand-500 bg-brand-50" : "border-line")}
        >
          {radio(mode === "uncategorized")}
          <span>
            <span className="block text-[13px] font-medium">Move bookmarks to Uncategorized</span>
            <span className="block text-xs text-ink-3">Sort them later</span>
          </span>
        </button>
        <div
          role="button"
          tabIndex={0}
          onClick={() => setMode("other")}
          onKeyDown={(e) => e.key === "Enter" && setMode("other")}
          className={clsx("flex gap-3 rounded-xl border p-3.5 text-left", mode === "other" ? "border-[1.5px] border-brand-500 bg-brand-50" : "border-line")}
        >
          {radio(mode === "other")}
          <span className="flex flex-1 flex-col gap-2">
            <span>
              <span className="block text-[13px] font-medium">Move bookmarks to another category</span>
              <span className="block text-xs text-ink-3">Keeps them organised under a new home</span>
            </span>
            {mode === "other" && <CategorySelect categories={categories} value={target} onChange={setTarget} noneLabel="Choose a category…" exclude={exclude} />}
          </span>
        </div>
        {subs.length > 0 && (
          <label className="flex items-center gap-2.5 text-[13px]">
            <input type="checkbox" checked={keepSubs} onChange={(e) => setKeepSubs(e.target.checked)} className="size-4 accent-brand-600" />
            Keep {subs.map((s) => s.name).join(" and ")} as top-level categories
          </label>
        )}
      </div>
    </Dialog>
  );
}
