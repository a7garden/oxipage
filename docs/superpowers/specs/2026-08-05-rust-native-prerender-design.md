# Rust-Native Markdown Prerender + Image Optimization (Option A) — Design

**Status:** Approved · **Date:** 2026-08-05

## Goal

Two build-time capabilities, both pure-Rust / single-binary / zero-Node:

1. **Prerender** — bake the rendered article body HTML into each content page's
   `index.html`, so crawlers and no-JS clients see the body text (SEO) and first
   paint shows content immediately. Zero changes to the React SPA mount.
2. **Image optimization** — for local media images, generate responsive WebP
   variants + intrinsic dimensions at build time, so both the prerendered HTML
   and the SPA emit `<img srcset width height>` (smaller payloads, no CLS).

Both preserve the single-binary, `cargo install → build` value proposition.

## Background — why A (not Astro)

Current state: `BuildExt::build_pages` emits a thin shell per content item —
`<div id="root">` + OG metas + SPA script; the body is rendered **client-side**
by the SPA's `markdown-it` from `/data/*.json`. Images are served **raw** with
only `loading="lazy"` + CSS `max-width`.

A was chosen over Astro because:

1. **Single-binary P0.** Astro's benefit requires `astro build` at content-build
   time (content JSON exists only then → can't be pre-embedded). Escape hatches —
   embedding Bun (~90 MB, unvalidated) or GH-Actions-bake (breaks
   `preview == production`) — each cost more than A.
2. **The body is markdown** — `pulldown-cmark` and Astro render equivalent HTML;
   Astro's edge for body rendering is marginal.
3. **JS is already small** — public SPA bundle is **768 KB** (gzip ~250 KB), so
   Astro's "ship less JS" is weak.

Astro's one concrete edge was **image optimization** (`<Image>` pipeline). The
user opted to capture that benefit under A via a Rust image pipeline rather than
adopt Astro. (Caveat: this helps **local media only**; external cover/thumbnail
URLs from TMDB/Aladin/OG remain raw — see Out of scope.)

## Architecture

### Prerender / SPA coexistence (core mechanism)

`build_pages` renders markdown→HTML and injects it as the **initial content**
inside `#root`:

```html
<div id="root">
  <article class="markdown">{rendered_markdown}</article>
</div>
<script src="/assets/index.js"></script>
```

The SPA mounts via `createRoot(document.getElementById('root')).render(...)`
(`web/src/main.tsx:6`) — a **fresh render that replaces `#root`'s children**. On
JS load React discards the prerendered article and renders its own view.

- **No hydration** → no mismatch errors.
- **Zero SPA code change** for the prerender itself.
- The prerendered HTML's job: be in the source for crawlers/no-JS + first paint.

### Shared markdown helper

- New module `oxibuilder-core::markdown` exposing
  `pub fn render(md: &str, asset_base: &str, images: &ImageManifest) -> String`.
- Dependency: `pulldown-cmark` (add to `oxibuilder-core`).
- Parser options to match the SPA's `markdown-it`: `ENABLE_TABLES`,
  `ENABLE_STRIKETHROUGH`, `ENABLE_TASKLISTS`, `ENABLE_FOOTNOTES`.

### Asset-path rewriting + image tags

`render()` rewrites logical `media/...` refs. For an image present in the
`ImageManifest`, it emits an optimized tag:

```html
<img src="{base}media/_derived/{hash}-960.webp"
     srcset="{base}media/_derived/{hash}-640.webp 640w, ... 1920w"
     width="960" height="540" loading="lazy" decoding="async" alt="...">
```

For non-media / external / unknown images it emits a plain `<img>` (unchanged
behavior). This mirrors the SPA's `useAssetResolver`
(`web/src/shared/asset-context.tsx`).

### Image pipeline (build-time, Rust)

- New module `oxibuilder-core::media::optimize`.
- Dependency: `image` crate (add to `oxibuilder-core`).
- Input: the set of local `media/...` paths referenced by content, the media
  source dir, and the output dir.
- For each image: decode → record intrinsic `width`/`height` → generate
  responsive variants at widths `{640, 960, 1280, 1920}` (capped at source
  width) → encode each as **WebP** → write to `out/media/_derived/{sha8}-{w}.webp`.
- **Cache by content hash:** `out/media/_derived/.cache.json` maps
  `{source_path, sha256}` → variant list. Builds skip regeneration when the
  source is unchanged and the derived files exist.
- Emit `/data/image-manifest.json`: `{ "media/foo.jpg": { "width": 960,
  "height": 540, "srcset": [{"w":640,"url":"media/_derived/...-640.webp"}, ...] } }`.
- Consumed by **both** the Rust prerenderer and the SPA markdown-it plugin
  (below) so JS users get optimized images too — otherwise `createRoot`'s
  replacement would swap optimized prerender for raw SPA images.

### SPA markdown-it image plugin

- A `markdown-it` plugin in `web/src/shared/Markdown.tsx` fetches
  `/data/image-manifest.json` once (cached), and rewrites `media/...` image
  tokens to the same optimized `<img srcset width height>` shape the prerender
  emits. External/unknown images pass through unchanged.

### Build wiring

- `build_writer::write_build_output` already derives `deployment_base`; thread it
  into the per-extension `build_pages` call (or resolve a placeholder token in
  the writer — impl detail).
- A new build step collects referenced local images across extensions, runs
  `media::optimize`, writes derived files + manifest, then each extension's
  `build_pages` renders with that manifest.

### Output changes

- `out/{ext}/{slug}/index.html` — now contains the rendered body inside `#root`.
- `out/media/_derived/*` — generated WebP variants + `.cache.json`.
- `out/data/image-manifest.json` — new.
- **Unchanged:** `index.md` (raw source), `index.json` (metadata),
  `/data/{ext}.json` (SPA data), search index.

## Scope

### v1 (this spec)

- Blog post body prerender (`oxibuilder-ext-blog`).
- markdown→HTML via `pulldown-cmark` + asset rewriting.
- Local-media image optimization: WebP variants + dimensions + `srcset`, with
  content-hash cache; manifest consumed by both prerender and SPA plugin.

### Out of scope (follow-up workstreams, documented)

- **External cover/thumbnail URLs** (TMDB/Aladin/OG) — remain raw; optimizing
  requires a download + rehost pipeline (network build dependency). Follow-up.
- **Structured React image fields** (BookCard/MovieCard covers, ProjectView
  screenshots) — the manifest exists for them to consume later; wiring the React
  components to it is a follow-up.
- **AVIF** — WebP only in v1; AVIF as a later addition.
- **Profile (bio) / project (description) prerender** — same prerender pattern;
  extend after blog lands.
- **List / lobby / search pages** prerender — lower SEO value; later.
- **Inline critical `.markdown` CSS** in the shell — start without; add only if
  a pre-CSS flash is felt.

## Verification

- **Unit (Rust):** `markdown::render()` golden tests (tables, fenced code,
  links, media-image → srcset/dims, external-image passthrough); `media::optimize`
  tests (variant count, dimensions, cache hit skips regen, manifest shape).
- **Integration (Rust):** extend `oxibuilder-core/tests/ssg_build.rs` — built
  blog `index.html` contains a body substring inside `#root`; derived WebP
  files + manifest exist for a referenced media image.
- **Unit (TS):** the markdown-it plugin rewrites a `media/...` token using a
  fixture manifest; external tokens pass through.
- **Manual:** `oxibuilder build` on a sample post with an image; `curl`
  `index.html` (body + optimized img present); browser confirms SPA hydrates and
  images load optimized; confirm `preview == production` still holds.

## Risks

- **markdown-it ↔ pulldown-cmark drift** on edge syntax — acceptable
  (`createRoot` replaces, not hydrates); affects only the brief pre-JS view.
  Document parity assumptions in code comments.
- **`image` crate decode/encode cost** at build time — mitigated by the
  content-hash cache; first build pays, subsequent builds skip.
- **`asset_base` plumbing** into `build_pages` — minor refactor; `build_writer`
  already owns base derivation.
- **Build-time image errors** (corrupt/unsupported source) — must not fail the
  whole build; log + fall back to the raw `media/...` URL for that image.
- **`index.md`** (raw source file) may no longer be consumed — verify; if
  unused, remove in a separate cleanup (not this spec).

## Decisions log

- **A over Astro** — single-binary P0 + marginal Astro edge for a markdown body.
- **Images included in v1** (local media) — user opted to capture Astro's image-
  optimization edge under A via a Rust `image` pipeline rather than adopt Astro.
  External URLs and structured React image fields are explicit follow-ups.
- **Blog-first prerender scope** — matches stated priority; incremental.
