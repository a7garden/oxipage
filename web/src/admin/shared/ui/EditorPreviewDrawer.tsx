import { useEffect, useState, type ReactNode } from "react";
import { createContext, useContext } from "react";
import { X } from "lucide-react";

import { cn } from "../../../shared/ui/cn";
import {
  adminAssetResolver,
  type AssetResolver,
} from "../../../shared/assets";

// Minimal in-file provider pair — when the peer plan ships the full
// PublicThemeScope + AssetResolverContext, the imports below can be
// redirected to the canonical locations without changing consumers.

const AssetResolverContext = createContext<AssetResolver | null>(null);

interface AssetResolverProviderProps {
  /** "admin" scopes by site slug; "public" falls back to document.baseURI. */
  mode: "admin" | "public";
  slug?: string;
  children: ReactNode;
}

/** Wraps the preview tree so *View/Card components resolve `media/...`
 *  through the admin endpoint (live preview). */
export function AssetResolverProvider({
  mode,
  slug,
  children,
}: AssetResolverProviderProps) {
  let resolver: AssetResolver;
  if (mode === "admin" && slug) {
    resolver = adminAssetResolver(slug);
  } else {
    // Public fallback: build a static resolver using the same safeUrl rules.
    resolver = {
      resolve(value) {
        if (!value) return null;
        if (/^(javascript|data|file|vbscript):/i.test(value.trim())) return null;
        try {
          return new URL(value, document.baseURI).toString();
        } catch {
          return null;
        }
      },
    };
  }
  return (
    <AssetResolverContext.Provider value={resolver}>
      {children}
    </AssetResolverContext.Provider>
  );
}

/** Consumer hook: *View components read the resolver off this context. */
export function useAssetResolver(): AssetResolver {
  return useContext(AssetResolverContext) ?? adminAssetResolver("");
}

interface PublicThemeScopeProps {
  children: ReactNode;
}

/** Scopes a subtree under the site's active `[data-public-theme]` so the public
 *  theme palette variables take effect (the Admin shell remains untouched).
 *  The attribute is published to <html> by `applyServerTheme`; fall back to
 *  "paper" before the first palette load. */
export function PublicThemeScope({ children }: PublicThemeScopeProps) {
  const themeId = document.documentElement.dataset.publicTheme ?? "paper";
  return (
    <div data-public-theme={themeId} className="contents">
      {children}
    </div>
  );
}

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
                tab === id ? "text-foreground border-b-2 border-[#22c55e]" : "text-muted",
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