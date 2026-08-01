// Shared asset-resolver context + public-theme scope.
//
// Promoted from a minimal in-file pair in EditorPreviewDrawer.tsx so that
// every media-bearing component — cover ImageFields, the public Markdown
// renderer, and the admin preview drawer — resolves `media/...` logical
// paths through ONE context. Three contexts are covered by two resolvers:
//   - "public"  → publicAssetResolver(): keys off document.baseURI. Works for
//                 the built/deployed site AND the live public preview because
//                 build_writer pins <base> to the deployment base and the
//                 preview handler overrides <base> to the preview prefix.
//   - "admin"   → adminAssetResolver(slug): the Admin SPA, where
//                 document.baseURI is the admin URL, so media must go through
//                 the per-site media endpoint.

import { createContext, useContext, type ReactNode } from "react";
import {
  adminAssetResolver,
  publicAssetResolver,
  type AssetResolver,
} from "./assets";

const AssetResolverContext = createContext<AssetResolver | null>(null);

interface AssetResolverProviderProps {
  /** "admin" scopes by site slug; "public" falls back to document.baseURI. */
  mode: "admin" | "public";
  slug?: string;
  children: ReactNode;
}

/** Wraps a subtree so media-bearing components resolve `media/...` through the
 *  correct context. */
export function AssetResolverProvider({
  mode,
  slug,
  children,
}: AssetResolverProviderProps) {
  const resolver =
    mode === "admin" && slug ? adminAssetResolver(slug) : publicAssetResolver();
  return (
    <AssetResolverContext.Provider value={resolver}>
      {children}
    </AssetResolverContext.Provider>
  );
}

/** Consumer hook. Defaults to publicAssetResolver() when no provider is
 *  mounted — the safe default for the public SPA, which renders at the
 *  deployment base. */
export function useAssetResolver(): AssetResolver {
  return useContext(AssetResolverContext) ?? publicAssetResolver();
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
