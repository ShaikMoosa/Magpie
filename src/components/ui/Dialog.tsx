import * as RD from "@radix-ui/react-dialog";
import { X } from "lucide-react";
import clsx from "clsx";
import type { ReactNode } from "react";

interface Props {
  open: boolean;
  onClose: () => void;
  title: string;
  subtitle?: string;
  icon?: ReactNode;
  iconClass?: string;
  width?: number;
  children: ReactNode;
  footer?: ReactNode;
  badge?: ReactNode;
}

export function Dialog({ open, onClose, title, subtitle, icon, iconClass, width = 600, children, footer, badge }: Props) {
  return (
    <RD.Root open={open} onOpenChange={(o) => !o && onClose()}>
      <RD.Portal>
        <RD.Overlay className="fixed inset-0 z-40 bg-overlay/40 backdrop-blur-[1px]" />
        <RD.Content
          style={{ width }}
          className="fixed top-1/2 left-1/2 z-50 flex max-h-[calc(100vh-48px)] -translate-x-1/2 -translate-y-1/2 flex-col overflow-hidden rounded-[18px] border border-line bg-canvas shadow-lg focus:outline-none"
          {...(subtitle ? {} : { "aria-describedby": undefined })}
        >
          <div className="flex items-center gap-2.5 border-b border-line px-6 py-4">
            {icon && (
              <div className={clsx("grid size-8 place-items-center rounded-[9px] [&_svg]:size-4", iconClass ?? "bg-brand-50 text-brand-600")}>
                {icon}
              </div>
            )}
            <div className="min-w-0 flex-1">
              <div className="flex items-center gap-2">
                <RD.Title className="text-base font-semibold tracking-tight">{title}</RD.Title>
                {badge}
              </div>
              {subtitle && <RD.Description className="truncate text-xs text-ink-3">{subtitle}</RD.Description>}
            </div>
            <RD.Close className="grid size-8 place-items-center rounded-lg text-ink-3 hover:bg-muted hover:text-ink" aria-label="Close">
              <X className="size-[18px]" />
            </RD.Close>
          </div>
          <div className="min-h-0 flex-1 overflow-y-auto">{children}</div>
          {footer && <div className="flex items-center gap-2.5 border-t border-line bg-subtle px-6 py-3.5">{footer}</div>}
        </RD.Content>
      </RD.Portal>
    </RD.Root>
  );
}
