import { useSyncExternalStore } from "react";

import {
  getImageManifest,
  type ImageManifest,
  type ManifestEntry,
  type ManifestSrc,
} from "./image-manifest";

/** Resolve an image URL (media ref or http(s)) to its optimized srcset/dims,
 *  or null when no manifest entry exists (live/preview fallback). */
export function useOptimizedImage(src: string | null | undefined): ManifestEntry | null {
  const m = useSyncExternalStore<ImageManifest>(
    () => () => {}, // no subscribe; manifest loads once at module init
    () => getImageManifest(), // snapshot
    () => ({} as ImageManifest), // SSR
  );
  if (!src) return null;
  const key = src.trim();
  return m[key] ?? null;
}

/** Largest variant ≤ 960px, else the largest available. */
export function pickVariant(entry: ManifestEntry): ManifestSrc {
  const sorted = [...entry.srcset].sort((a, b) => a.w - b.w);
  return sorted.find((v) => v.w <= 960) ?? sorted[sorted.length - 1] ?? entry.srcset[0];
}
