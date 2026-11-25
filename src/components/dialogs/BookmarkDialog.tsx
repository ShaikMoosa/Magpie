import { AlertTriangle, Bookmark as BookmarkIcon, Check, ImageIcon, Link2, Loader2, Pencil, Star, Trash2 } from "lucide-react";
import clsx from "clsx";
import { useEffect, useState } from "react";
import { useCategories, useCreateBookmark, useUpdateBookmark } from "../../hooks/queries";
import { api, errorMessage, isAppError } from "../../lib/api";
import { UNCATEGORIZED_COLOR } from "../../lib/colors";
import { formatDate, formatRelative } from "../../lib/format";
import type { Bookmark, PageMeta, UrlCheck } from "../../lib/types";
import { toast } from "../../store/toast";
import { usePrefs, useUi } from "../../store/ui";
import { Favicon, Thumbnail } from "../bookmarks/visuals";
import { useBookmarkActions } from "../bookmarks/useBookmarkActions";
import { SecurityBadge } from "../ui/Badges";
import { Button } from "../ui/Button";
import { Dialog } from "../ui/Dialog";
import { Field, Kbd, TextArea, TextInput, Toggle } from "../ui/Field";
import { CategorySelect } from "./CategorySelect";

function useUrlCheck(url: string, excludeId?: string) {
  const [check, setCheck] = useState<UrlCheck | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    if (!url.trim()) {
      setCheck(null);
      setError(null);
      return;
    }
    const t = setTimeout(() => {
      api
        .checkUrl(url, excludeId)
        .then((c) => {
          setCheck(c);
          setError(null);
        })
        .catch((e) => {
          setCheck(null);
          setError(errorMessage(e));
        });
    }, 300);
    return () => clearTimeout(t);
  }, [url, excludeId]);
  return { check, error };
}

function parseTags(s: string): string[] {
  return s
    .split(/[,\s]+/)
    .map((t) => t.replace(/^#/, "").trim())
    .filter(Boolean);
}

export function BookmarkDialog({ mode, bookmark, initialUrl, onClose }: { mode: "add" | "edit"; bookmark?: Bookmark; initialUrl?: string; onClose: () => void }) {
  const { data: categories = [] } = useCategories();
  const create = useCreateBookmark();
  const update = useUpdateBookmark();
  const a = useBookmarkActions();
  const route = useUi((s) => s.route);

  const defaultCategory = route.page === "library" && route.view === "category" ? (route.categoryId ?? null) : null;
  const [url, setUrl] = useState(bookmark?.url ?? initialUrl ?? "");
  const [title, setTitle] = useState(bookmark?.title ?? "");
  const [description, setDescription] = useState(bookmark?.description ?? "");
  const [categoryId, setCategoryId] = useState<string | null>(bookmark ? bookmark.categoryId : defaultCategory);
  const [isFavourite, setFavourite] = useState(bookmark?.isFavourite ?? false);
  const [tags, setTags] = useState((bookmark?.tags ?? []).map((t) => `#${t}`).join(" "));
  const [allowDuplicate, setAllowDuplicate] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const { check, error } = useUrlCheck(url, bookmark?.id);
  const autoPreviews = usePrefs((s) => s.autoPreviews);
  const [meta, setMeta] = useState<PageMeta | null>(null);
  const [metaState, setMetaState] = useState<"idle" | "loading" | "done" | "failed">("idle");
  const [metaError, setMetaError] = useState<string | null>(null);
  const [touched, setTouched] = useState({ title: false, description: false });

  // Fetch the page preview for new bookmarks (the Rust side refuses private addresses).
  const previewUrl = mode === "add" && autoPreviews && check && check.securityStatus !== "dangerous" ? check.url : null;
  useEffect(() => {
    if (!previewUrl) {
      setMeta(null);
      setMetaState("idle");
      return;
    }
    let live = true;
    setMetaState("loading");
    api
      .fetchMetadata(previewUrl)
      .then((m) => {
        if (!live) return;
        setMeta(m);
        setMetaState("done");
      })
      .catch((e) => {
        if (!live) return;
        setMeta(null);
        setMetaError(errorMessage(e));
        setMetaState("failed");
      });
    return () => {
      live = false;
    };
  }, [previewUrl]);
  useEffect(() => {
    if (!meta) return;
    if (!touched.title && meta.title) setTitle(meta.title);
    if (!touched.description && meta.description) setDescription(meta.description);
    // Only react to newly fetched metadata.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [meta]);

  const duplicate = check?.duplicateOf && check.duplicateOf !== bookmark?.id;
  const busy = create.isPending || update.isPending;
  const canSave = !!check && !error && (!duplicate || allowDuplicate || mode === "edit") && !busy;
  const category = categories.find((c) => c.id === categoryId);

  const save = async () => {
    setSaveError(null);
    try {
      if (mode === "add") {
        const b = await create.mutateAsync({
          url,
          title: title || null,
          description: description || null,
          categoryId,
          isFavourite,
          tags: parseTags(tags),
          allowDuplicate,
          thumbnail: meta?.image ?? null,
          favicon: meta?.favicon ?? null,
        });
        if (!meta && autoPreviews) void api.refreshMetadata([b.id]).catch(() => undefined);
        toast({ type: "success", title: "Bookmark saved", body: `${b.title} added to ${category?.name ?? "Uncategorized"}` });
      } else if (bookmark) {
        await update.mutateAsync({ id: bookmark.id, url, title, description: description || null, categoryId, isFavourite, tags: parseTags(tags) });
        toast({ type: "success", title: "Changes saved" });
      }
      onClose();
    } catch (e) {
      setSaveError(isAppError(e) && e.code === "DUPLICATE_BOOKMARK" ? "This URL is already in your library." : errorMessage(e));
    }
  };

  const urlStatus = error ? (
    <span className="text-xs text-danger">{error}</span>
  ) : check ? (
    <span className="inline-flex items-center gap-1 rounded-full bg-success-bg px-2 py-0.5 text-xs font-medium whitespace-nowrap text-success">
      <Check className="size-3" /> Valid URL
    </span>
  ) : null;

  const form = (
    <div className="flex flex-col gap-4 px-6 py-5">
      <Field label="Website URL" hint={mode === "edit" ? "Changing the URL re-checks security." : undefined}>
        <TextInput
          autoFocus
          leading={<Link2 />}
          value={url}
          onChange={(e) => setUrl(e.target.value)}
          placeholder="https://"
          invalid={!!error}
          trailing={check ? urlStatus : undefined}
          onKeyDown={(e) => e.key === "Enter" && canSave && void save()}
        />
        {error && urlStatus}
      </Field>

      {check && mode === "add" && (
        <div className="overflow-hidden rounded-[14px] border border-line">
          <div className="flex gap-3.5 p-3">
            <Thumbnail color={category?.color ?? UNCATEGORIZED_COLOR} file={meta?.image} className="h-[92px] w-[132px]" />
            <div className="min-w-0 flex-1">
              <p className="flex items-center gap-2 text-sm font-medium">
                <Favicon domain={check.domain} file={meta?.favicon} size={18} />
                <span className="truncate">{title || meta?.title || check.domain}</span>
              </p>
              <p className="truncate text-xs text-ink-3">{check.url}</p>
              {metaState === "loading" ? (
                <p className="mt-1 flex items-center gap-1.5 text-xs text-ink-3">
                  <Loader2 className="size-3 animate-spin" /> Fetching title, description and preview…
                </p>
              ) : metaState === "failed" ? (
                <p className="mt-1 text-xs text-ink-3">Preview unavailable: {metaError} You can still save it.</p>
              ) : !autoPreviews ? (
                <p className="mt-1 text-xs text-ink-3">Automatic previews are off in Settings.</p>
              ) : (
                <p className="mt-1 line-clamp-2 text-xs text-ink-2">{description || meta?.description || "No description found."}</p>
              )}
            </div>
          </div>
          <div className="flex flex-wrap items-center gap-x-4 gap-y-1.5 border-t border-line bg-subtle px-3.5 py-2.5 text-xs text-ink-2">
            {duplicate ? (
              <span className="flex items-center gap-1.5 font-medium text-warning">
                <AlertTriangle className="size-3.5" /> Already in your library
              </span>
            ) : (
              <span className="flex items-center gap-1.5 font-medium">
                <Check className="size-3.5 text-success" /> Not a duplicate
              </span>
            )}
            <SecurityBadge status={check.securityStatus} />
            <span className="text-ink-3">Local checks · just now</span>
          </div>
          {check.securityReasons.length > 0 && (
            <ul className="border-t border-line px-3.5 py-2.5 text-xs text-ink-2">
              {check.securityReasons.map((r) => (
                <li key={r} className="list-inside list-disc">
                  {r}
                </li>
              ))}
            </ul>
          )}
          {duplicate && (
            <label className="flex items-center gap-2 border-t border-line px-3.5 py-2.5 text-xs">
              <input type="checkbox" checked={allowDuplicate} onChange={(e) => setAllowDuplicate(e.target.checked)} className="accent-brand-600" />
              Save a second copy anyway
            </label>
          )}
        </div>
      )}

      <div className="grid grid-cols-2 gap-3.5">
        <Field label="Title">
          <TextInput value={title} onChange={(e) => {
              setTitle(e.target.value);
              setTouched((t) => ({ ...t, title: true }));
            }} placeholder={check?.domain ?? "Uses the domain if empty"} maxLength={300} />
        </Field>
        <Field label="Category">
          <CategorySelect categories={categories} value={categoryId} onChange={setCategoryId} />
        </Field>
      </div>
      <Field label="Description">
        <TextArea value={description} onChange={(e) => {
            setDescription(e.target.value);
            setTouched((t) => ({ ...t, description: true }));
          }} rows={2} maxLength={1000} placeholder="Optional" />
      </Field>
      <div className="grid grid-cols-[1fr_auto] items-end gap-3.5">
        <Field label="Tags" hint="Separate with spaces or commas">
          <TextInput value={tags} onChange={(e) => setTags(e.target.value)} placeholder="#inspiration #ui" />
        </Field>
        <label className="mb-6 flex h-10 items-center gap-2.5 rounded-[10px] border border-line-strong px-3 text-sm">
          <Star className={clsx("size-4", isFavourite ? "fill-warning text-warning" : "text-ink-3")} />
          Favourite
          <Toggle checked={isFavourite} onChange={setFavourite} label="Favourite" />
        </label>
      </div>
      {saveError && <p className="text-[13px] text-danger">{saveError}</p>}
    </div>
  );

  const footer = (
    <>
      {mode === "edit" && bookmark ? (
        <Button
          variant="ghost"
          className="!text-danger"
          icon={<Trash2 />}
          onClick={() => {
            a.trashNow([bookmark.id]);
            onClose();
          }}
        >
          Move to Trash
        </Button>
      ) : (
        <span className="text-xs text-ink-3">Saved locally on this PC.</span>
      )}
      <div className="flex-1" />
      <Button onClick={onClose}>Cancel</Button>
      <Button variant="primary" icon={<Check />} disabled={!canSave} onClick={() => void save()}>
        {mode === "add" ? "Save bookmark" : "Save changes"}
      </Button>
    </>
  );

  if (mode === "add") {
    return (
      <Dialog open onClose={onClose} title="Add bookmark" icon={<BookmarkIcon />} width={600} footer={footer} badge={<Kbd>Ctrl N</Kbd>}>
        {form}
      </Dialog>
    );
  }

  return (
    <Dialog open onClose={onClose} title="Edit bookmark" subtitle={bookmark?.title} icon={<Pencil />} width={800} footer={footer}>
      <div className="flex">
        <div className="min-w-0 flex-1">{form}</div>
        {bookmark && (
          <aside className="flex w-[280px] shrink-0 flex-col gap-4 border-l border-line bg-subtle p-5 text-xs">
            <span className="font-medium text-ink-2">Preview</span>
            <Thumbnail color={category?.color ?? UNCATEGORIZED_COLOR} file={bookmark.thumbnailPath} className="h-[140px] w-full" />
            <Button size="sm" icon={<ImageIcon />} onClick={() => a.refreshPreview([bookmark.id])}>
              Refresh preview
            </Button>
            <div className="flex flex-col gap-1.5">
              <span className="font-medium text-ink-2">Security</span>
              <SecurityBadge status={bookmark.securityStatus} className="self-start" />
              <span className="text-ink-3">
                {bookmark.securitySource ?? "Local heuristics"} · {formatRelative(bookmark.securityCheckedAt)}
              </span>
              {bookmark.securityReasons.map((r) => (
                <span key={r} className="text-ink-2">
                  • {r}
                </span>
              ))}
            </div>
            <dl className="flex flex-col gap-1.5">
              <span className="font-medium text-ink-2">Details</span>
              {[
                ["Added", formatDate(bookmark.createdAt)],
                ["Modified", formatDate(bookmark.updatedAt)],
                ["Last opened", formatRelative(bookmark.lastOpenedAt)],
                ["Opened", `${bookmark.openCount} times`],
                ["Source", bookmark.sourceBrowser ? `${bookmark.sourceBrowser}${bookmark.sourceFolder ? ` › ${bookmark.sourceFolder}` : ""}` : "Added manually"],
              ].map(([k, v]) => (
                <div key={k} className="flex justify-between gap-2">
                  <dt className="text-ink-3">{k}</dt>
                  <dd className="truncate text-right font-medium">{v}</dd>
                </div>
              ))}
            </dl>
          </aside>
        )}
      </div>
    </Dialog>
  );
}
