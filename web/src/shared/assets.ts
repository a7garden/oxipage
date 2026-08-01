// Asset resolvers — convert a stored reference (logical path or external
// URL) into a URL the current context can fetch.
//
// Three resolvers cover the four contexts (live preview, built preview,
// built public, live admin):
//   - adminAssetResolver(slug):  media/...   → /api/console/s/{slug}/media/...
//   - previewAssetResolver(p):   media/...   → new URL(mediaPath, p)
//   - publicAssetResolver():     media/...   → new URL(mediaPath, document.baseURI)
//
// Absolute http(s) URLs pass through unchanged. Unsupported schemes
// (javascript:, data:, file:) are rejected and resolve to null so the
// caller can fall back to a neutral placeholder.

export interface AssetResolver {
  resolve(value: string | null | undefined): string | null;
}

const UNSUPPORTED_SCHEMES = /^(javascript|data|file|vbscript):/i;

function safeUrl(value: string): URL | null {
  try {
    return new URL(value);
  } catch {
    return null;
  }
}

function isHttpish(url: URL): boolean {
  return url.protocol === "http:" || url.protocol === "https:";
}

function isUnsupported(value: string): boolean {
  return UNSUPPORTED_SCHEMES.test(value.trim());
}

function normalize(mediaPath: string): string {
  // Strip a single leading slash so callers can store either "media/x" or "/media/x".
  return mediaPath.replace(/^\/+/, "");
}

export function adminAssetResolver(slug: string): AssetResolver {
  const base = `/api/console/s/${slug}/`;
  return {
    resolve(value) {
      if (!value) return null;
      if (isUnsupported(value)) return null;
      const u = safeUrl(value);
      if (u && isHttpish(u)) return value;
      return base + normalize(value);
    },
  };
}

export function previewAssetResolver(previewBase: string): AssetResolver {
  const base = previewBase.endsWith("/") ? previewBase : previewBase + "/";
  return {
    resolve(value) {
      if (!value) return null;
      if (isUnsupported(value)) return null;
      const u = safeUrl(value);
      if (u && isHttpish(u)) return value;
      try {
        return new URL(normalize(value), base).toString();
      } catch {
        return null;
      }
    },
  };
}

export function publicAssetResolver(): AssetResolver {
  return {
    resolve(value) {
      if (!value) return null;
      if (isUnsupported(value)) return null;
      const u = safeUrl(value);
      if (u && isHttpish(u)) return value;
      try {
        return new URL(normalize(value), document.baseURI).toString();
      } catch {
        return null;
      }
    },
  };
}
