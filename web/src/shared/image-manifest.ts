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

/** Path the SPA fetches. The build writer drops the manifest at
 * `<base>/data/image-manifest.json` (e.g. `/blog/data/...` under a project
 * deploy). MUST be relative so the browser resolves it against `<base href>`;
 * an absolute-path URL (leading `/`) would resolve against the document
 * ORIGIN and 404 in every project deployment. */
const MANIFEST_URL = "data/image-manifest.json";

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

/** True for `media/...` logical refs OR external http(s) URLs. The Rust build
 *  pipeline emits both into the image manifest — `media/...` from inline-markdown
 *  refs (Task 1) and full external URLs (TMDB posters / Aladin covers, Task 2)
 *  — so the SPA must resolve both with the same lookup path. */
export function isOptimizableRef(src: string): boolean {
  const s = src.trim();
  return /^\/?media\//.test(s) || /^https?:\/\//.test(s);
}

/** Resolve the deployment `<base href>` so the emitted `<img>` URLs match
 *  the Rust prerender after the browser merges them against `<base href>`.
 *
 *  Returns the base WITH its slashes as read from `<base href>`:
 *    `<base href="/blog/">` → `"/blog/"`
 *    `<base href="/">`      → `"/"`
 *    no `<base>` (vite dev) → `"/"`
 *
 *  `resolveMedia` then concatenates `${base}${url}` (no separator) — the base
 *  already ends in `/` and the manifest url is `media/_derived/...` (no
 *  leading slash), so the result is absolute and matches the Rust output
 *  exactly: `/blog/media/_derived/...` (project) and `/media/_derived/...`
 *  (apex). Emitting a RELATIVE URL here would double the path under
 *  `<base href="/blog/">` (e.g. `blog/media/x.webp` → `/blog/blog/media/...`).
 */
export function deploymentBasePrefix(): string {
  if (typeof document === "undefined") return "/";
  return document.querySelector("base")?.getAttribute("href") ?? "/";
}

/** Build the optimized `<img>` string for a `media/...` ref that lives in
 *  the loaded manifest, or `null` if it does not (caller falls back to the
 *  plain image token render). `base` is the deployment base WITH its
 *  surrounding slashes (see [`deploymentBasePrefix`]) — the function
 *  concatenates `${base}${url}` so the emitted URL is absolute and matches
 *  the Rust prerender after the browser merges it against `<base href>`. */
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
  const srcUrl = `${base}${chosen.url}`;
  const srcset = entry.srcset
    .map((e) => `${base}${e.url} ${e.w}w`)
    .join(", ");
  return `<img src="${srcUrl}" srcset="${srcset}" width="${entry.width}" height="${entry.height}" loading="lazy" decoding="async" alt="">`;
}

/** Module-level cache the synchronous markdown rule reads. The async
 *  `loadImageManifest` resolves into this var at module load; until it
 *  lands, the rule sees `{}` and falls back to the existing resolver path. */
let manifest: ImageManifest = {};

// Fire the fetch at module load — markdown-it renders synchronously, so the
// rule can only read whatever's already in `manifest` when it's invoked. On
// the built site the manifest lives at `<base>/data/image-manifest.json` (a
// relative URL so the browser resolves it against `<base href>`); on the
// rare cold-start race the rule falls back to plain `<img>` (current
// behavior), so there's no visual regression.
loadImageManifest().then((m) => {
  manifest = m;
});

/** Synchronous accessor for the loaded manifest. Used by the markdown-it
 *  image rule. Returns `{}` before the fetch resolves. */
export function getImageManifest(): ImageManifest {
  return manifest;
}
