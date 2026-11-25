import { Link2, ShieldAlert, ShieldCheck, Trash2 } from "lucide-react";
import { useState, type ReactNode } from "react";
import { useEmptyTrash, useCounts, useMarkSafe, useSecuritySettings } from "../../hooks/queries";
import { displayUrl, formatRelative } from "../../lib/format";
import type { Bookmark } from "../../lib/types";
import { toast } from "../../store/toast";
import { useBookmarkActions } from "../bookmarks/useBookmarkActions";
import { Button } from "../ui/Button";
import { Dialog } from "../ui/Dialog";

/** Shown when opening a suspicious or dangerous bookmark. */
export function OpenWarningDialog({ bookmark, onClose }: { bookmark: Bookmark; onClose: () => void }) {
  const a = useBookmarkActions();
  const markSafe = useMarkSafe();
  const dangerous = bookmark.securityStatus === "dangerous";
  const { data: sec } = useSecuritySettings();
  const blocked = dangerous && (sec?.config.blockDangerous ?? true);
  const [understood, setUnderstood] = useState(false);
  return (
    <Dialog
      open
      onClose={onClose}
      title={blocked ? "Magpie blocked this website" : dangerous ? "This website is a known threat" : "This link looks suspicious"}
      icon={<ShieldAlert />}
      iconClass={dangerous ? "bg-danger-bg text-danger" : "bg-warning-bg text-warning"}
      width={540}
      footer={
        <>
          <button
            className="text-[13px] font-medium whitespace-nowrap text-ink-2 hover:text-ink"
            onClick={() => {
              void navigator.clipboard.writeText(bookmark.url);
              toast({ type: "success", title: "URL copied" });
            }}
          >
            Copy URL
          </button>
          <button
            className="text-[13px] font-medium whitespace-nowrap text-ink-2 hover:text-ink"
            onClick={() => {
              markSafe.mutate([bookmark.id]);
              onClose();
            }}
          >
            Mark as safe
          </button>
          <div className="flex-1" />
          <Button
            variant="ghost"
            className="!text-danger"
            disabled={blocked && !understood}
            onClick={() => {
              a.openConfirmed(bookmark);
              onClose();
            }}
          >
            Open anyway…
          </Button>
          <Button variant="primary" icon={<ShieldCheck />} onClick={onClose}>
            Keep me safe
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-3.5 px-6 py-5">
        <p className="text-sm text-ink-2">
          {dangerous
            ? "This bookmark is a known threat. Opening it could install malware or steal your data."
            : "Security checks found warning signs. Make sure you trust this site before opening it."}
        </p>
        <div className={`flex flex-col gap-2 rounded-xl border px-3.5 py-3 ${dangerous ? "border-red-200 bg-danger-bg" : "border-amber-200 bg-warning-bg"}`}>
          <span className={`flex items-center gap-1.5 text-[13px] font-medium ${dangerous ? "text-danger" : "text-warning"}`}>
            <Link2 className="size-3.5" /> {displayUrl(bookmark.url)}
          </span>
          <ul className="flex flex-col gap-1 text-xs">
            {bookmark.securityReasons.map((r) => (
              <li key={r}>• {r}</li>
            ))}
          </ul>
          <span className="text-xs text-ink-3">
            {bookmark.securitySource ?? "Local heuristics"} · {formatRelative(bookmark.securityCheckedAt)}
          </span>
        </div>
        {blocked && (
          <label className="flex items-center gap-2 text-xs text-ink-2">
            <input type="checkbox" checked={understood} onChange={(e) => setUnderstood(e.target.checked)} className="accent-red-600" />
            I understand the risk and want the option to open it anyway
          </label>
        )}
      </div>
    </Dialog>
  );
}

export function Confirm({ title, body, confirmLabel, onConfirm, onClose, icon }: { title: string; body: ReactNode; confirmLabel: string; onConfirm: () => void; onClose: () => void; icon: ReactNode }) {
  return (
    <Dialog
      open
      onClose={onClose}
      title={title}
      icon={icon}
      iconClass="bg-danger-bg text-danger"
      width={460}
      footer={
        <>
          <div className="flex-1" />
          <Button onClick={onClose}>Cancel</Button>
          <Button
            variant="danger"
            autoFocus
            onClick={() => {
              onConfirm();
              onClose();
            }}
          >
            {confirmLabel}
          </Button>
        </>
      }
    >
      <p className="px-6 py-5 text-sm text-ink-2">{body}</p>
    </Dialog>
  );
}

export function ConfirmTrashDialog({ ids, onClose }: { ids: string[]; onClose: () => void }) {
  const a = useBookmarkActions();
  return (
    <Confirm
      title={`Move ${ids.length} bookmarks to Trash?`}
      body="You can restore them from Trash until it's emptied."
      confirmLabel="Move to Trash"
      icon={<Trash2 />}
      onConfirm={() => a.trashNow(ids)}
      onClose={onClose}
    />
  );
}

export function EmptyTrashDialog({ onClose }: { onClose: () => void }) {
  const empty = useEmptyTrash();
  const { data: counts } = useCounts();
  const n = counts?.trash ?? 0;
  return (
    <Confirm
      title="Empty Trash?"
      body={`${n} bookmark${n === 1 ? "" : "s"} will be permanently deleted from this PC. This can't be undone.`}
      confirmLabel={`Delete ${n} permanently`}
      icon={<Trash2 />}
      onConfirm={() => empty.mutate(undefined, { onSuccess: (k) => toast({ type: "success", title: `Deleted ${k} bookmarks permanently` }) })}
      onClose={onClose}
    />
  );
}
