import { useEffect, useState, type ReactNode } from "react";
import { X } from "lucide-react";

import { cn } from "../../../shared/ui/cn";
import {
  AssetResolverProvider,
  PublicThemeScope,
} from "../../../shared/asset-context";

interface EditorPreviewDrawerProps {
  open: boolean;
  onClose: () => void;
  title: string;
  description?: string;
  width?: string; // tailwind class for the editor pane on desktop
  dirty?: boolean;
  onRequestClose?: () => boolean | void;
  editor: React.ReactNode;
  preview: React.ReactNode;
  footer?: React.ReactNode;
  /** Slug for the asset resolver scope (admin mode). */
  slug?: string;
}

/// 2-pane editor + preview on desktop (≥ md). Smaller viewports collapse to
/// Edit/Preview tabs. The preview pane is wrapped in the site's PublicTheme
/// and the site's admin asset resolver so media URLs resolve identically to
/// the published site.
export function EditorPreviewDrawer({
  open,
  onClose,
  title,
  description,
  width = "w-[460px]",
  dirty = false,
  onRequestClose,
  editor,
  preview,
  footer,
  slug,
}: EditorPreviewDrawerProps) {
  const [tab, setTab] = useState<"edit" | "preview">("edit");

  // Reset to Edit whenever the drawer opens.
  useEffect(() => {
    if (open) setTab("edit");
  }, [open]);

  // Escape close hook: route through dirty/confirm guard.
  useEffect(() => {
    if (!open) return;
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") attemptClose();
    }
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, dirty]);

  function attemptClose() {
    if (dirty && !window.confirm("Discard unsaved changes?")) return;
    if (onRequestClose && onRequestClose() === false) return;
    onClose();
  }

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-50 flex bg-black/40">
      <div className="flex-1" onClick={attemptClose} aria-hidden />

      {/* Desktop: 2-pane. Mobile: single pane with tabs. */}
      <div
        className={cn(
          "bg-canvas border-l border-line h-full overflow-hidden flex flex-col shadow-2xl",
          width,
        )}
        role="dialog"
        aria-modal="true"
        aria-label={title}
      >
        <header className="flex items-start justify-between px-5 py-4 border-b border-line">
          <div>
            <h2 className="text-base font-semibold text-foreground">{title}</h2>
            {description && <p className="text-xs text-muted mt-0.5">{description}</p>}
          </div>
          <button
            onClick={attemptClose}
            className="inline-flex items-center justify-center size-7 rounded-md text-muted hover:text-foreground hover:bg-surface/50"
            aria-label="Close"
          >
            <X size={16} />
          </button>
        </header>

        {/* Mobile tabs (hidden md:flex). */}
        <div className="flex border-b border-line md:hidden">
          {(["edit", "preview"] as const).map((id) => (
            <button
              key={id}
              onClick={() => setTab(id)}
              className={cn(
                "flex-1 px-4 py-2 text-sm font-medium capitalize",
                tab === id ? "text-primary border-b-2 border-active" : "text-muted",
              )}
            >
              {id}
            </button>
          ))}
        </div>

        <div className="flex-1 min-h-0 flex">
          {/* Editor pane: hidden on mobile when Preview tab is active. */}
          <div
            className={cn(
              "overflow-y-auto px-5 py-4",
              "w-full md:w-[var(--editor-w,460px)] md:shrink-0 md:border-r md:border-line",
              tab === "preview" ? "hidden md:block" : "block",
            )}
          >
            {editor}
          </div>

          {/* Preview pane: scoped to the site's public theme + admin resolver. */}
          <div
            className={cn(
              "flex-1 min-w-0 overflow-y-auto bg-surface/40",
              tab === "edit" ? "hidden md:block" : "block",
            )}
          >
            <PublicThemeScope>
              <AssetResolverProvider mode="admin" slug={slug}>
                {preview}
              </AssetResolverProvider>
            </PublicThemeScope>
          </div>
        </div>

        {footer && (
          <div className="px-5 py-4 border-t border-line flex justify-end gap-2 bg-surface/30">
            {footer}
          </div>
        )}
      </div>
    </div>
  );
}