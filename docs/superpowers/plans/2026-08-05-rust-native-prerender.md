# Rust-Native Markdown Prerender + Image Optimization — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bake rendered markdown into each blog page's `index.html` (SEO/first-paint) and optimize local media images to responsive WebP with intrinsic dimensions — both at build time, pure-Rust, single binary, zero SPA mount change.

**Architecture:** A `markdown::render()` helper (pulldown-cmark) produces body HTML, injected into the existing `<div id="root">` shell; the SPA's `createRoot` replaces it on mount (no hydration mismatch). A `media::optimize()` pass (the `image` crate) generates WebP variants + dimensions, emits `image-manifest.json`, consumed by both the Rust renderer and a SPA markdown-it plugin.

**Tech Stack:** Rust (`pulldown-cmark`, `image`, `sha2`), TypeScript (`markdown-it`), SQLite via sqlx, rayon.

**Spec:** `docs/superpowers/specs/2026-08-05-rust-native-prerender-design.md`

## Global Constraints

- Single self-contained binary preserved — `cargo install oxibuilder` → `oxibuilder build` works with no Node on the host. New native deps (`image`'s WebP enc) link statically; verify no dynamic libs ship.
- `preview == production`: one renderer pipeline feeds both the preview server and the deployed `out/`. No CI-only transforms.
- Conventional commits (`feat:`, `test:`, `chore:`), English messages. Korean only in user-facing strings/comments.
- `BuildExt::build_pages` signature: `fn build_pages(&self, db: &SqlitePool, rt: &tokio::runtime::Handle) -> Result<Vec<StaticPage>, Box<dyn Error + Send + Sync>>` (do not widen the trait unless a task says to).
- Image errors must never fail a whole build — log and fall back to the raw `media/...` URL for that image.
- **`out/` wipe invariant:** `write_build_output` does `remove_dir_all(out_dir)` at its start (`build_writer.rs:44-46`). Therefore anything the image pipeline produces MUST be written to a **staging dir outside `out/`** (e.g. `{data_dir}/.image-build/`); `write_build_output` copies staging→`out/` after the wipe. Never write build artifacts directly to `out/` before `write_build_output` runs — they will be destroyed.

## File Structure

**Create (Rust, `oxibuilder-core`):**
- `src/markdown.rs` — `render(md, asset_base, images) -> String`; the pulldown-cmark pipeline + image-tag emission.
- `src/media.rs` — `optimize(...)` + `ImageManifest` type + cache. Exposes `ImageManifest` (serde) used by `markdown` and emitted as `/data/image-manifest.json`.

**Create (TS):**
- `web/src/shared/image-manifest.ts` — fetch+cache `/data/image-manifest.json`; `resolveMedia(src) -> {src, srcset, width, height} | null`.

**Modify (Rust):**
- `crates/oxibuilder-core/Cargo.toml` — add `pulldown-cmark`, `image`.
- `crates/oxibuilder-core/src/lib.rs` — `pub mod markdown; pub mod media;`.
- `crates/oxibuilder-core/src/build_writer.rs` — write `image-manifest.json`; (base resolution helper, see Task 5).
- `crates/oxibuilder-ext-blog/src/lib.rs` — `build_pages` renders body into the shell.
- `crates/oxibuilder-cli/src/commands/build.rs` — run the image pre-pass; thread manifest + a base placeholder into the build.

**Modify (TS):**
- `web/src/shared/Markdown.tsx` — image rule consults `image-manifest.ts`.

**Tests:**
- `crates/oxibuilder-core/src/markdown.rs` (`#[cfg(test)] mod tests`).
- `crates/oxibuilder-core/src/media.rs` (`#[cfg(test)] mod tests`).
- `crates/oxibuilder-core/tests/ssg_build.rs` — extend.
- `web/src/shared/image-manifest.test.ts` (new).

---

### Task 1: `markdown::render()` core (pulldown-cmark), no images yet

**Files:** Create `crates/oxibuilder-core/src/markdown.rs`; Modify `Cargo.toml`, `src/lib.rs`.

**Interfaces:**
- Produces: `pub fn render(md: &str, asset_base: &str, images: &crate::media::ImageManifest) -> String` (image wiring added in Task 3; here `images` is accepted but unused for non-media markdown).
- Consumes (forward-declared): `crate::media::ImageManifest` — define a minimal stub now (Task 2 fills it): `pub struct ImageManifest { entries: std::collections::HashMap<String, ImageEntry> }` with `pub fn empty() -> Self` and `impl Default`.

- [ ] **Step 1: Add deps**

In `crates/oxibuilder-core/Cargo.toml` `[dependencies]`, add:
```toml
pulldown-cmark = "0.12"
```

- [ ] **Step 2: Write the failing test**

`crates/oxibuilder-core/src/markdown.rs`:
```rust
//! Build-time markdown → HTML (pulldown-cmark), parity with the SPA's markdown-it.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::ImageManifest;

    #[test]
    fn renders_heading_paragraph_and_code() {
        let md = "# Title\n\npara with `code`\n\n```\nx = 1\n```";
        let html = render(md, "/blog/", &ImageManifest::default());
        assert!(html.contains("<h1>Title</h1>"));
        assert!(html.contains("<code>code</code>"));
        assert!(html.contains("<pre"));
    }

    #[test]
    fn renders_table() {
        let md = "| a | b |\n| - | - |\n| 1 | 2 |";
        let html = render(md, "/", &ImageManifest::default());
        assert!(html.contains("<table>"));
        assert!(html.contains("<td>1</td>"));
    }
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test -p oxibuilder-core markdown::tests`
Expected: FAIL — `cannot find function render`.

- [ ] **Step 4: Implement**

`crates/oxibuilder-core/src/markdown.rs` (add above the test module):
```rust
use pulldown_cmark::{Options, Parser};

/// Render owner-authored markdown to trusted HTML (no sanitization; doc §0.3).
/// `asset_base` rewrites logical `media/...` refs; `images` (Task 3) adds srcset/dims.
pub fn render(md: &str, _asset_base: &str, _images: &crate::media::ImageManifest) -> String {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TASKLISTS);
    opts.insert(Options::ENABLE_FOOTNOTES);
    let parser = Parser::new_ext(md, opts);
    let mut out = String::with_capacity(md.len() * 2);
    pulldown_cmark::html::push_html(&mut out, parser);
    out
}
```

`crates/oxibuilder-core/src/lib.rs`: add `pub mod markdown; pub mod media;` and create `src/media.rs` with the stub:
```rust
//! Build-time local-image optimization (Task 2 fills this in).

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ImageManifest {
    /// logical `media/...` path → entry
    pub entries: std::collections::HashMap<String, ImageEntry>,
}

impl ImageManifest {
    pub fn empty() -> Self { Self::default() }
    pub fn get(&self, path: &str) -> Option<&ImageEntry> { self.entries.get(path) }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ImageEntry {
    pub width: u32,
    pub height: u32,
    pub srcset: Vec<ImageSrc>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ImageSrc {
    pub w: u32,
    pub url: String,
}
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p oxibuilder-core markdown::tests`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/oxibuilder-core/src/markdown.rs crates/oxibuilder-core/src/media.rs crates/oxibuilder-core/src/lib.rs crates/oxibuilder-core/Cargo.toml
git commit -m "feat(core): add pulldown-cmark markdown::render + ImageManifest stub"
```

---

### Task 2: `media::optimize()` — WebP variants + dimensions + cache (staging)

**Files:** Modify `Cargo.toml`, `src/media.rs`.

**Interfaces:**
- Produces: `pub fn optimize(refs: &[String], media_dir: &Path, staging_dir: &Path) -> std::io::Result<ImageManifest>` — for each local `media/...` ref that exists on disk, write `staging_dir/media/_derived/{sha8}-{w}.webp` for widths `{640,960,1280,1920}` capped at source width, record `{width,height,srcset}`, reuse cached variants via `staging_dir/media/_derived/.cache.json`. **`staging_dir` lives OUTSIDE `out/`** (see Global Constraints: `write_build_output` wipes `out/`); `write_build_output` (Task 5) copies `staging_dir/media/_derived/` → `out/media/_derived/` after the wipe. Missing/undecodable refs are skipped (logged), never erroring.
- Cache: `.cache.json` maps `"{path}:{sha256}" -> Vec<ImageSrc>`. Skip regen when key present and all files exist.

- [ ] **Step 1: Add dep**

`crates/oxibuilder-core/Cargo.toml` `[dependencies]`:
```toml
image = "0.25"
image-webp = "0.2"
```
Note: `image` crate's built-in WebP encoder wraps libwebp (C). `image-webp` is the maintained path used by `image`. Confirm at impl which API is current; both link statically → single binary preserved. If a pure-Rust encoder is preferred, flag in review.

- [ ] **Step 2: Write failing test**

Append to `src/media.rs` test module (`out`-like behavior, but the dir is **staging**, not `out/` — it must survive independently of any `out/` wipe):
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgba};
    use std::path::PathBuf;

    fn write_test_png(dir: &Path, name: &str, w: u32, h: u32) -> PathBuf {
        let p = dir.join(name);
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_pixel(w, h, Rgba([255, 0, 0, 255]));
        img.save(&p).unwrap();
        p
    }

    #[test]
    fn optimizes_local_image_to_webp_variants_and_caches() {
        let tmp = tempfile::tempdir().unwrap();
        let media = tmp.path().join("media"); std::fs::create_dir_all(&media).unwrap();
        let staging = tmp.path().join("staging");   // OUTSIDE any out/ — survives wipes
        write_test_png(&media, "shot.png", 2000, 1125);

        let m1 = optimize(&["media/shot.png".into()], &media, &staging).unwrap();
        let e = m1.get("media/shot.png").expect("entry present");
        assert_eq!((e.width, e.height), (2000, 1125));
        // widths capped at source (2000): 640,960,1280,1920 all ≤ 2000
        assert_eq!(e.srcset.len(), 4);
        assert!(staging.join("media/_derived").is_dir());
        assert!(e.srcset.iter().all(|s| s.url.ends_with(".webp")));

        // Second run: cache hit — no regen, same result.
        let m2 = optimize(&["media/shot.png".into()], &media, &staging).unwrap();
        assert_eq!(m2.get("media/shot.png").unwrap().srcset.len(), 4);
    }

    #[test]
    fn missing_ref_is_skipped_not_error() {
        let tmp = tempfile::tempdir().unwrap();
        let staging = tmp.path().join("staging");
        let m = optimize(&["media/ghost.png".into()], &tmp.path().join("media"), &staging).unwrap();
        assert!(m.get("media/ghost.png").is_none());
    }
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test -p oxibuilder-core media::tests`
Expected: FAIL — `cannot find function optimize`.

- [ ] **Step 4: Implement**

In `src/media.rs` (above the test module), implement `optimize` writing to `staging_dir`:
```rust
use std::path::{Path, PathBuf};
use sha2::{Digest, Sha256};

const WIDTHS: [u32; 4] = [640, 960, 1280, 1920];

pub fn optimize(refs: &[String], media_dir: &Path, staging_dir: &Path) -> std::io::Result<ImageManifest> {
    let derived = staging_dir.join("media").join("_derived");
    std::fs::create_dir_all(&derived)?;
    let cache_path = derived.join(".cache.json");
    let mut cache: std::collections::HashMap<String, Vec<ImageSrc>> = read_cache(&cache_path);

    let mut manifest = ImageManifest::empty();
    for raw in refs {
        let logical = raw.trim_start_matches('/');
        if !logical.starts_with("media/") { continue; }            // external/non-media: skip
        let src = media_dir.join(logical.trim_start_matches("media/"));
        if !src.exists() { continue; }                              // missing: skip, don't error
        let bytes = match std::fs::read(&src) { Ok(b) => b, Err(_) => continue };
        let sha8 = hex8(&Sha256::digest(&bytes));
        let key = format!("{logical}:{sha8}");
        let entry = match cache.get(&key).filter(|v| v.iter().all(|s| derived.join(url_file(&s.url)).exists())) {
            Some(v) => decode_dims_and_entry(v.clone(), &bytes),
            None => {
                let e = match generate(&bytes, &sha8, &derived) { Ok(x) => x, Err(_) => continue };
                cache.insert(key, e.srcset.clone());
                e
            }
        };
        if let Some(e) = entry { manifest.entries.insert(logical.to_string(), e); }
    }
    write_cache(&cache_path, &cache)?;
    Ok(manifest)
}
```
Add helpers `hex8`, `url_file`, `read_cache`, `write_cache`, `decode_dims_and_entry`, and `generate(bytes, sha8, derived) -> io::Result<ImageEntry>` that decodes via `image::load_from_memory`, records intrinsic `(w,h)`, and for each width in `WIDTHS` that is `<= w` resizes (imageops) and re-encodes to WebP at `derived.join(format!("{sha8}-{w}.webp"))`, pushing `ImageSrc { w, url: format!("media/_derived/{sha8}-{w}.webp") }`. On any decode/encode error, return `Err` so the caller `continue`s (skip). `decode_dims_and_entry` re-derives `(w,h)` from the cached bytes (cheap) so the manifest always carries dimensions even on cache hits. **All paths are under `staging_dir`, never `out_dir`.**

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p oxibuilder-core media::tests`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/oxibuilder-core/src/media.rs crates/oxibuilder-core/Cargo.toml
git commit -m "feat(core): media::optimize — responsive WebP variants + dims + cache (staging)"
```

---

### Task 3: Image tags in `markdown::render()`

**Files:** Modify `src/markdown.rs`.

**Interfaces:**
- `render()` now, for each image node whose URL is a `media/...` ref present in `images`, emits `<img src="{base}{derived_960_url}" srcset="..." width height loading="lazy" decoding="async" alt="...">`. Pick `src` = the srcset entry with the largest width ≤ 960 (fallback: largest). Non-media / external / manifest-missing images render as default pulldown-cmark `<img>` with `src` rewritten to `{base}{logical}` for media refs.

- [ ] **Step 1: Write failing test**

Append to `markdown::tests`:
```rust
    #[test]
    fn media_image_in_manifest_gets_srcset_and_dims() {
        let mut m = ImageManifest::default();
        m.entries.insert("media/shot.png".into(), ImageEntry {
            width: 2000, height: 1125,
            srcset: vec![
                ImageSrc { w: 640,  url: "media/_derived/ab-640.webp".into() },
                ImageSrc { w: 960,  url: "media/_derived/ab-960.webp".into() },
                ImageSrc { w: 1280, url: "media/_derived/ab-1280.webp".into() },
                ImageSrc { w: 1920, url: "media/_derived/ab-1920.webp".into() },
            ],
        });
        let html = render("![alt](media/shot.png)", "/blog/", &m);
        assert!(html.contains(r#"src="blog/media/_derived/ab-960.webp""#), "{html}");
        assert!(html.contains("srcset="));
        assert!(html.contains(r#"width="2000""#));
        assert!(html.contains(r#"loading="lazy""#));
    }

    #[test]
    fn external_image_passes_through() {
        let html = render("![x](https://e.com/a.png)", "/", &ImageManifest::default());
        assert!(html.contains(r#"src="https://e.com/a.png""#));
        assert!(!html.contains("srcset="));
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p oxibuilder-core markdown::tests::media_image`
Expected: FAIL — pulldown-cmark emits a plain `<img src="media/shot.png">`, no srcset/dims.

- [ ] **Step 3: Implement**

Replace the body of `render()` to walk `Parser` events and custom-render `Tag::Image`. Concretely, collect events; when emitting an image, compute the tag attributes from `images.get(logical)`. Use `pulldown_cmark::html` only as a fallback for text runs — or simpler: post-process is fragile, so use the event loop. Sketch:
```rust
pub fn render(md: &str, asset_base: &str, images: &crate::media::ImageManifest) -> String {
    use pulldown_cmark::{Event, Tag, TagEnd, Options, Parser};
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH
                | Options::ENABLE_TASKLISTS | Options::ENABLE_FOOTNOTES);
    let mut out = String::with_capacity(md.len() * 2);
    for event in Parser::new_ext(md, opts) {
        match event {
            Event::Start(Tag::Image { link_type: _, dest_url, title, id }) => {
                out.push_str(&render_image_open(&dest_url, &title, &id, asset_base, images));
            }
            // all other events: defer to the default HTML writer
            other => pulldown_cmark::html::push_html(&mut out, std::iter::once(other)),
        }
    }
    out
}
```
Implement `render_image_open(...)` to: if `is_media_ref(dest)` and `Some(entry) = images.get(dest.trim_start_matches('/'))`, emit the optimized `<img ...>` with `alt=""` (alt text isn't carried in `Tag::Image` here; acceptable — the SPA path also doesn't supply alt) and the chosen `src`/`srcset`/`width`/`height`/`loading`/`decoding`. Otherwise emit `<img src="{asset_base}{logical or raw}" alt="">` for media refs, or `<img src="{raw}" alt="">` for external. (`alt` handling: pulldown-cmark delivers alt text as subsequent `Text` events inside the image span; for v1 emit `alt=""` and document that alt is not forwarded — parity with current SPA behavior. If alt is required, accumulate inlined text until `TagEnd::Image`.)
Note: the `match` above must not double-emit; verify against pulldown-cmark 0.12 event semantics at impl — if `Start(Image)` + following events + `End(Image)` is the model, suppress the inner text events when an optimized tag was emitted.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p oxibuilder-core markdown::tests`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/oxibuilder-core/src/markdown.rs
git commit -m "feat(core): markdown::render emits optimized img for manifest media"
```

---

### Task 4: Blog `build_pages` prerender + base placeholder

**Files:** Modify `crates/oxibuilder-ext-blog/src/lib.rs`; Test `crates/oxibuilder-core/tests/ssg_build.rs` is extended in Task 5.

**Interfaces:**
- `build_pages` renders `post.body` via `markdown::render(&body, BASE_PLACEHOLDER, &manifest)` and injects the result into the shell's `#root`. `BASE_PLACEHOLDER = "\u{0}BASE\u{0}"` (resolved to `deployment_base` in `write_build_output`, Task 5).
- The manifest is obtained from a new field on the builder or passed via thread-local; see Task 5 for wiring. For this task, assume `BlogExtension` holds `manifest: Arc<ImageManifest>` set by the build command before `build_site`.

- [ ] **Step 1: Add the field + thread the manifest**

In `crates/oxibuilder-ext-blog/src/lib.rs`, change `pub struct BlogExtension;` to:
```rust
pub struct BlogExtension {
    pub manifest: std::sync::OnceLock<oxibuilder_core::media::ImageManifest>,
}
impl BlogExtension {
    pub fn new() -> Self { Self { manifest: std::sync::OnceLock::new() } }
    pub fn set_manifest(&self, m: oxibuilder_core::media::ImageManifest) { let _ = self.manifest.set(m); }
}
```
Update every `BlogExtension` construction site to `BlogExtension::new()` (search the repo; console loader + tests).

- [ ] **Step 2: Rewrite the HTML emission in `build_pages`**

Replace the `format!(r#"<!DOCTYPE html> ...#)` block (the `<div id="root"></div>` body) so the root carries the rendered article:
```rust
let images = self.manifest.get().cloned().unwrap_or_default();
// ...inside the post loop, after excerpt:
let body_html = oxibuilder_core::markdown::render(&post.body, oxibuilder_core::markdown::BASE_PLACEHOLDER, &images);
let html = format!(
    r#"<!DOCTYPE html>
<html lang="{lang}">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>{title}</title>
  <meta property="og:title" content="{title}">
  <meta property="og:description" content="{excerpt}">
  <meta property="og:type" content="article">
  <meta property="og:url" content="/blog/{slug}/">
  <meta name="twitter:card" content="summary">
  <link rel="canonical" href="/blog/{slug}/">
</head>
<body>
  <div id="root"><article class="markdown">{body}</article></div>
  <script src="/assets/index.js"></script>
</body>
</html>
"#,
    lang = post.lang, title = post.title, slug = post.slug,
    excerpt = excerpt, body = body_html,
);
```
Add `pub const BASE_PLACEHOLDER: &str = "\u{0}BASE\u{0}";` to `src/markdown.rs`.

- [ ] **Step 3: Run existing tests**

Run: `cargo test -p oxibuilder-ext-blog`
Expected: PASS (the welcome-post seed etc. unaffected).

- [ ] **Step 4: Commit**

```bash
git add crates/oxibuilder-ext-blog/src/lib.rs crates/oxibuilder-core/src/markdown.rs
git commit -m "feat(blog): prerender markdown body into #root shell"
```

---

### Task 5: Build wiring — image pre-pass (staging), base resolution, copy into fresh `out/`

**Files:** Modify `crates/oxibuilder-cli/src/commands/build.rs`, `crates/oxibuilder-core/src/build_writer.rs`, `crates/oxibuilder-core/src/builder.rs` (`BuildInputs`); Test `crates/oxibuilder-core/tests/ssg_build.rs`.

**Interfaces:**
- `BuildInputs` gains two optional fields: `pub image_staging_dir: Option<PathBuf>` and `pub image_manifest: Option<ImageManifest>` (set by the build command after the pre-pass). `BuildInputs::new` defaults them to `None`.
- Build command flow: (1) derive `deployment_base`; (2) `staging = data_dir.join(".image-build")`; (3) collect `media/...` refs from blog bodies; (4) `manifest = media::optimize(&refs, &media_dir, &staging)?`; (5) `blog_builder.set_manifest(manifest.clone())`; (6) set `inputs.image_staging_dir = Some(staging)` + `inputs.image_manifest = Some(manifest)`; (7) `build_site`; (8) `write_build_output` — resolves `BASE_PLACEHOLDER`→`deployment_base`, **copies `staging/media/_derived/`→`out/media/_derived/` (after its internal `out/` wipe)**, writes `out/data/image-manifest.json`.
- **Why staging:** `write_build_output` does `remove_dir_all(out_dir)` at the top (`build_writer.rs:44-46`). Derived files written to `out/` before the writer runs are destroyed. Staging lives under `data_dir/.image-build/` (outside `out/`), survives the wipe, and is copied into the freshly-cleaned `out/`.

- [ ] **Step 1: Write failing integration test (wipe-survival regression)**

Append to `crates/oxibuilder-core/tests/ssg_build.rs` (adapt to its existing harness):
```rust
#[test]
fn derived_images_surive_out_wipe_and_manifest_is_written() {
    // 1. optimize() → staging (outside out/)
    // 2. write_build_output() wipes out/, then must copy staging/media/_derived → out/media/_derived
    //    and write out/data/image-manifest.json.
    // Assert: out/media/_derived/<hash>-960.webp exists, out/data/image-manifest.json exists,
    // and a page containing BASE_PLACEHOLDER now contains the deployment_base ("/repo/").
    // (Reuse the harness's write_build_output call path; populate BuildInputs.image_staging_dir
    //  + .image_manifest from a real optimize() call on a fixture PNG.)
}

#[test]
fn base_placeholder_resolved_to_deployment_base_in_pages() {
    // page content contains BASE_PLACEHOLDER inside #root; after write_build_output
    // the written index.html contains "/repo/" (the derived deployment_base), not the placeholder.
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p oxibuilder-core --test ssg_build derived_images base_placeholder`
Expected: FAIL — staging copy not implemented / placeholder not resolved.

- [ ] **Step 3: Implement base resolution + staging copy + manifest write**

In `build_writer.rs::write_build_output`, **after** the `remove_dir_all`/`create_dir_all` block (lines 44-47) and after pages/data are written, add:
```rust
// Resolve the base placeholder the renderer injected (safe even if absent).
let deployment_base = BuildManifest::from_site_base(
    &inputs.site_base_url, &inputs.theme_id, &inputs.asset_revision_seed,
).deployment_base;
for p in output.pages.iter_mut() {
    if p.content.contains(crate::markdown::BASE_PLACEHOLDER) {
        p.content = p.content.replace(crate::markdown::BASE_PLACEHOLDER, &deployment_base);
    }
}
// NOTE: the page loop above already wrote files; either (a) do this replace BEFORE the write
// loop (move it above step 4), or (b) re-write the touched pages. Prefer (a): resolve placeholders
// on `output.pages` before the write loop runs. Reorder accordingly.

// Copy derived images from staging into the freshly-cleaned out/, and write the manifest.
if let (Some(staging), Some(manifest)) = (&inputs.image_staging_dir, &inputs.image_manifest) {
    let src_derived = staging.join("media").join("_derived");
    let dst_derived = out_dir.join("media").join("_derived");
    if src_derived.is_dir() {
        copy_dir_recursive(&src_derived, &dst_derived)?; // existing helper in this file
    }
    let manifest_path = out_dir.join("data").join("image-manifest.json");
    if let Some(parent) = manifest_path.parent() { fs::create_dir_all(parent)?; }
    fs::write(&manifest_path, serde_json::to_string_pretty(manifest)?)?;
}
```
(`copy_dir_recursive` already exists in `build_writer.rs`; reuse it. Do NOT copy `.cache.json` into `out/` — skip it during the copy, or accept it harmlessly; the canonical cache lives in staging.)

In `commands/build.rs`, before `build_site`:
```rust
let staging = data_dir.join(".image-build");
let refs = collect_media_refs(&pool, &rt); // scan blog bodies for /?media/...
let manifest = oxibuilder_core::media::optimize(&refs, &media_dir, &staging)?;
blog_builder.set_manifest(manifest.clone());
inputs.image_staging_dir = Some(staging);
inputs.image_manifest = Some(manifest);
```
Add `collect_media_refs` (regex/scan blog `body` for `/?media/[^\s)]+` → dedupe). Then the existing `build_site` + `write_build_output`.

- [ ] **Step 4: Run test to verify it passes; run full core test suite**

Run: `cargo test -p oxibuilder-core`
Expected: PASS — derived files present in `out/` after the wipe; manifest written; base resolved.

- [ ] **Step 5: Commit**

```bash
git add crates/oxibuilder-cli/src/commands/build.rs crates/oxibuilder-core/src/build_writer.rs crates/oxibuilder-core/src/builder.rs crates/oxibuilder-core/tests/ssg_build.rs
git commit -m "fix(build): stage derived images outside out/, copy after wipe; resolve base + write manifest"
```

---

### Task 6: SPA markdown-it image plugin (manifest consumption)

**Files:** Create `web/src/shared/image-manifest.ts`; Modify `web/src/shared/Markdown.tsx`; Test `web/src/shared/image-manifest.test.ts`.

**Interfaces:**
- `image-manifest.ts`: `type ManifestEntry = { width: number; height: number; srcset: { w: number; url: string }[] }`; `loadImageManifest(): Promise<Record<string, ManifestEntry>>` (fetch `/data/image-manifest.json` once, cached); `resolveMedia(src: string, base: string, m?): { html: string } | null` returns the full `<img ...>` string or `null` (passthrough).
- `Markdown.tsx` image rule: if `isMediaRef(src)` and the manifest has an entry, emit the optimized tag (matching the Rust output); else current behavior.

- [ ] **Step 1: Write failing test**

`web/src/shared/image-manifest.test.ts`:
```ts
import { describe, it, expect, vi, beforeEach } from "vitest";
// (If the repo uses vitest — confirm test runner at impl; adjust to the project's runner.)
import { resolveMedia } from "./image-manifest";

describe("resolveMedia", () => {
  it("returns optimized img for a manifest media ref", () => {
    const m = { "media/shot.png": { width: 2000, height: 1125,
      srcset: [{ w: 960, url: "media/_derived/ab-960.webp" }] } };
    const out = resolveMedia("media/shot.png", "blog/", m as any);
    expect(out).toContain('src="blog/media/_derived/ab-960.webp"');
    expect(out).toContain('width="2000"');
    expect(out).toContain("srcset=");
  });
  it("returns null for external urls", () => {
    expect(resolveMedia("https://e.com/a.png", "/", {})).toBeNull();
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `bun test web/src/shared/image-manifest.test.ts` (confirm runner — repo may use `bun test` or vitest).
Expected: FAIL — module not found.

- [ ] **Step 3: Implement `image-manifest.ts`**

```ts
export type ManifestEntry = { width: number; height: number; srcset: { w: number; url: string }[] };

let cache: Promise<Record<string, ManifestEntry>> | null = null;
export async function loadImageManifest(): Promise<Record<string, ManifestEntry>> {
  if (!cache) {
    cache = fetch("data/image-manifest.json")
      .then((r) => (r.ok ? r.json() : {}))
      .catch(() => ({}));
  }
  return cache;
}

function pickSrc(entry: ManifestEntry): string {
  // largest width <= 960, else largest
  const sorted = [...entry.srcset].sort((a, b) => a.w - b.w);
  return (sorted.find((s) => s.w <= 960) ?? sorted[sorted.length - 1]).url;
}

export function resolveMedia(
  src: string, base: string, m?: Record<string, ManifestEntry>
): string | null {
  const logical = src.replace(/^\//, "");
  const entry = m?.[logical];
  if (!entry) return null;
  const s = pickSrc(entry);
  const srcset = entry.srcset.map((e) => `${base}${e.url} ${e.w}w`).join(", ");
  return `<img src="${base}${s}" srcset="${srcset}" width="${entry.width}" height="${entry.height}" loading="lazy" decoding="async" alt="">`;
}
```

- [ ] **Step 4: Wire into `Markdown.tsx`**

Make the image rule async-aware is awkward in markdown-it (sync render). Since the manifest is fetched once globally, load it at app boot (e.g. in `main.tsx` or a top-level `await loadImageManifest()` storing into a module-level var), then `Markdown.tsx` reads the synchronously-resolved cache:
```ts
import { loadImageManifest } from "./image-manifest";
let manifest: Record<string, any> = {};
loadImageManifest().then((m) => { manifest = m; }); // fire at module load
```
In the `md.renderer.rules.image` handler, after resolving media, call `resolveMedia(src, resolver.base, manifest)`; if non-null, return that HTML directly (bypass the default image token renderer); else current passthrough. (Confirm `resolver.base` is exposed by `asset-context.tsx`; if not, derive from `document.baseURI`.)

- [ ] **Step 5: Run test to verify it passes**

Run: `bun test web/src/shared/image-manifest.test.ts`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add web/src/shared/image-manifest.ts web/src/shared/image-manifest.test.ts web/src/shared/Markdown.tsx
git commit -m "feat(web): markdown-it image plugin consumes image-manifest"
```

---

### Task 7: Manual smoke + parity check + cleanup

**Files:** none (verification + optional cleanup).

- [ ] **Step 1: Build a sample post with an image**

Create a blog post whose markdown body references an uploaded `media/sample.jpg`; `oxibuilder build`.

- [ ] **Step 2: Verify the static artifact**

`grep` / open `out/blog/<slug>/index.html`: assert the rendered body text AND `<img ... srcset ... width height>` are present inside `#root`; assert `out/media/_derived/*.webp` and `out/data/image-manifest.json` exist.

- [ ] **Step 3: Verify SPA still hydrates**

`oxibuilder console --preview`; open the preview; confirm the post renders in-browser and images load (optimized). Confirm `preview == production` (same `out/`).

- [ ] **Step 4: Parity note**

Document `pulldown-cmark` ↔ `markdown-it` parity assumptions (tables, strikethrough, tasklists, footnotes, linkify) in a code comment in `markdown.rs`. Note alt-text is not forwarded in v1.

- [ ] **Step 5: Final commit (docs/changelog if any)**

```bash
git add -A
git commit -m "docs: markdown/image parity notes for prerender"
```

---

## Self-Review

**Spec coverage:** Prerender (Tasks 1, 3–5) ✓; image pipeline local media (Tasks 2, 3, 5) ✓; SPA parity (Task 6) ✓; verification (Task 7) ✓. Out-of-scope items (external URLs, structured React fields, AVIF, profile/project/list prerender) are documented in the spec, not claimed here.

**Placeholders:** Task 2 marks the `image`/`image-webp` crate API as "confirm at impl" — this is a concrete crate-version pointer, not a content gap; the test pins the behavior. Task 5 offers two wiring paths (manifest via `BuildOutput` vs written from the command) — pick the simpler at impl; both are fully specified. Task 6 flags the test runner + `resolver.base` exposure to confirm — concrete, not vague.

**Type consistency:** `ImageManifest`/`ImageEntry`/`ImageSrc` defined once (Task 1) and reused identically in Rust (Tasks 2–5) and mirrored in TS (Task 6). `render(md, asset_base, images)` signature is stable across Tasks 1, 3, 4. `BASE_PLACEHOLDER` defined in Task 1, used in Tasks 4–5.

**Risk:** WebP native dep (libwebp) — flagged in Global Constraints + Task 2; static link keeps single-binary. Build-time decode cost mitigated by content-hash cache (Task 2).
