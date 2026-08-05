// SPA-side image manifest loader + `<img>` tag builder.
//
// Mirrors the Rust build's responsive-`<img>` output so JS-enabled users get
// the same WebP variants + width/height hints the prerendered HTML has.
// Source of truth for the attribute set is `crates/oxibuilder-core/src/markdown.rs`
// (`render_image_open`); this module reproduces it in TypeScript so the
// markdown-it renderer can substitute the optimized tag when a `media/...`
// ref appears in the loaded manifest.

/** One variant inside a manifest entry. Mirrors `media::ImageSrc`. */
export interface ManifestSrc {
  /** Pixel width of the variant. */
  w: number;
  /** Relative URL of the variant (`media/_derived/{sha8}-{w}.webp` in v1). */
  url: string;
}

/** A single image's intrinsic dims + its responsive variant list. */
export interface ManifestEntry {
  width: number;
  height: number;
  srcset: ManifestSrc[];
}

/** The full manifest: `media/...` logical ref → entry. */
export type ImageManifest = Record<string, ManifestEntry>;

/** Path the SPA fetches. The build writer drops the manifest at `out/data/`
 *  and the `<base href>` makes `/data/...` resolve to the deployed location. */
const MANIFEST_URL = "/data/image-manifest.json";

let cache: Promise<ImageManifest> | null = null;

/** Fetch the image manifest once and cache the promise. Returns `{}` on any
 *  failure (404 in dev, malformed JSON, network error) so the markdown rule
 *  silently falls back to the non-optimized path. */
export function loadImageManifest(): Promise<ImageManifest> {
  if (!cache) {
    cache = fetch(MANIFEST_URL)
      .then((r) => (r.ok ? (r.json() as Promise<ImageManifest>) : {}))
      .catch(() => ({} as ImageManifest));
  }
  return cache;
}

/** Largest variant whose width is ≤ 960; fallback to the largest available.
 *  Matches Rust `pick_src` (`crates/oxibuilder-core/src/markdown.rs`). */
function pickSrc(entry: ManifestEntry): ManifestSrc {
  let best: ManifestSrc | undefined;
  for (const s of entry.srcset) {
    if (s.w <= 960 && (best === undefined || s.w > best.w)) {
      best = s;
    }
  }
  if (best !== undefined) return best;
  // Rust falls back to `entry.srcset.last()` — the manifest is emitted in
  // ascending width order (`WIDTHS = [640, 960, 1280, 1920]`), so the last
  // entry is the widest available variant.
  return entry.srcset[entry.srcset.length - 1];
}

/** True for `media/...` logical references (with or without a leading slash). */
export function isMediaRef(src: string): boolean {
  return /^\/?media\//.test(src.trim());
}

/** Resolve the manifest's deployment-base prefix from `<base href>` so the
 *  emitted URLs match the Rust prerender. Returns `""` when no `<base>` is
 *  present (vite dev) or when the href is just `/`. */
export function deploymentBasePrefix(): string {
  if (typeof document === "undefined") return "";
  const href = document.querySelector("base")?.getAttribute("href") ?? "/";
  // Mirror Rust's `asset_base.trim_matches('/')` — the `<base href>` is
  // always `/` or `/<path>/`, so this yields `""` or `"<path>"`.
  return href.replace(/^\/+|\/+$/g, "");
}

/** Build the optimized `<img>` string for a `media/...` ref that lives in
 *  the loaded manifest, or `null` if it does not (caller falls back to the
 *  plain image token render). `base` is the already-trimmed deployment
 *  prefix (see [`deploymentBasePrefix`]). */
export function resolveMedia(
  src: string,
  base: string,
  m?: ImageManifest,
): string | null {
  if (!isMediaRef(src)) return null;
  const logical = src.replace(/^\/+/, "");
  const entry = m?.[logical];
  if (!entry) return null;
  // Empty srcset (Task 2's `media::generate` skips widths larger than the
  // source — icons/thumbnails narrower than 640px land here). Mirror Rust's
  // `if !entry.srcset.is_empty()` gate (`crates/oxibuilder-core/src/markdown.rs`)
  // by falling through to the plain `<img>` render in Markdown.tsx; emitting
  // an empty srcset would crash `pickSrc` (no element at `length - 1`).
  if (entry.srcset.length === 0) return null;
  const chosen = pickSrc(entry);
  // Strip leading/trailing slashes so callers can pass either `"/blog/"`,
  // `"/blog"`, or `"blog/"` — matches Rust's `asset_base.trim_matches('/')`.
  // The literal `/` separator added below then yields the same string Rust
  // emits: prefix="" + "/" + "media/..." = "/media/..." (apex);
  // prefix="blog" + "/" + "media/..." = "blog/media/..." (project).
  const prefix = base.replace(/^\/+|\/+$/g, "");
  const srcUrl = `${prefix}/${chosen.url}`;
  const srcset = entry.srcset
    .map((e) => `${prefix}/${e.url} ${e.w}w`)
    .join(", ");
  return `<img src="${srcUrl}" srcset="${srcset}" width="${entry.width}" height="${entry.height}" loading="lazy" decoding="async" alt="">`;
}

/** Module-level cache the synchronous markdown rule reads. The async
 *  `loadImageManifest` resolves into this var at module load; until it
 *  lands, the rule sees `{}` and falls back to the existing resolver path. */
let manifest: ImageManifest = {};

// Fire the fetch at module load — markdown-it renders synchronously, so the
// rule can only read whatever's already in `manifest` when it's invoked. On
// the built site the manifest lives at `/data/image-manifest.json` and is
// typically resolved by the time Markdown first renders; in the rare cold-
// start race the rule falls back to plain `<img>` (current behavior), so
// there's no visual regression.
loadImageManifest().then((m) => {
  manifest = m;
});

/** Synchronous accessor for the loaded manifest. Used by the markdown-it
 *  image rule. Returns `{}` before the fetch resolves. */
export function getImageManifest(): ImageManifest {
  return manifest;
}
