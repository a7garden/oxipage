# External Image Build-Time Optimization — Implementation Plan (Track B)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Optimize external poster/cover images (TMDB, Aladin, Google Books) at build time into local WebP variants — closing the gap with blog-test's `cache-posters`/`cache-book-covers` pipeline — by extending the existing `media.rs` infrastructure rather than adding a parallel one.

**Architecture:** Add an HTTP-download path to `media.rs` (`optimize_external`) keyed on the source URL. Add a `BuildExt::external_image_urls` hook (default-empty) so movies/books contribute their poster/cover URLs. `run_image_pre_pass` merges local refs + external URLs into one manifest. The front-end broadens `isMediaRef` to recognize external URLs and resolves them to local srcsets, falling back to the raw CDN URL when no manifest entry exists (live/preview mode).

**Tech Stack:** Rust (`reqwest`, `image`, `sha2`, `sqlx`), TypeScript/React.

## Global Constraints

- WebP-only (decision: reuse existing `media.rs`; no AVIF/JPEG this pass).
- Network failures are **skipped, never errored** — mirror the existing "missing ref skipped" contract. A transient TMDB outage must not fail the build.
- Cache key is SHA-256(bytes); the same poster image served from different URL widths reuses its variants.
- Manifest key namespace: local refs keep `media/...`; external URLs are keys verbatim (`https://...`). No collision (`media/` vs `https://`).
- `BuildExt` is `Send + Sync`; the new method needs a **default impl** so existing implementors compile untouched.
- Above-the-fold N (≈10) cards load `eager`+`fetchPriority="high"`; the rest `lazy`.
- Commit convention: conventional commits, English.

---

## File Structure

**Rust — image pipeline:**
- Modify: `crates/oxibuilder-core/src/media.rs` (new `optimize_external`, reuse `generate`)
- Modify: `crates/oxibuilder-core/src/builder.rs` (`BuildExt::external_image_urls` default)
- Modify: `crates/oxibuilder-core/src/build.rs` (`run_image_pre_pass` merges external)

**Rust — extensions:**
- Modify: `crates/oxibuilder-ext-movies/src/lib.rs` (`MoviesExtension::external_image_urls`)
- Modify: `crates/oxibuilder-ext-books/src/lib.rs` (`BooksExtension::external_image_urls`)

**Rust — build wiring:**
- Modify: `crates/oxibuilder-cli/src/commands/build.rs` (pass builders slice to pre-pass)
- Modify: `crates/oxibuilder-console/src/build/build_run.rs` (same)

**Front-end:**
- Modify: `web/src/shared/image-manifest.ts` (`isOptimizableRef`, `resolveMedia`)
- Create: `web/src/shared/useOptimizedImage.ts` (hook: url → srcset/img attrs)
- Modify: `web/src/extensions/movies/MovieCard.tsx`, `MovieDetailPage.tsx`
- Modify: `web/src/extensions/books/BookCard.tsx`

---

### Task 1: `media::optimize_external` — HTTP download → WebP

**Files:**
- Modify: `crates/oxibuilder-core/src/media.rs`
- Test: `crates/oxibuilder-core/src/media.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Produces: `pub async fn optimize_external(urls: &[String], staging_dir: &Path, http: &reqwest::Client) -> io::Result<ImageManifest>` — manifest keys are the URLs verbatim.

- [ ] **Step 1: Write the failing test**

In `media.rs` tests module, add (using a local file served via `data:` is not possible with reqwest; instead test the byte-path directly by extracting a helper — see Step 2 — and test that helper):

```rust
#[tokio::test]
async fn optimize_external_downloads_and_keys_by_url() {
    // Spin a tiny mock server returning fixed PNG bytes, OR test the extracted
    // `generate_from_bytes` helper directly (preferred — no network in unit test).
    let tmp = tempfile::tempdir().unwrap();
    let staging = tmp.path();
    let png = make_test_png(100, 150); // existing test helper, if any; else 1x1 png bytes
    let http = reqwest::Client::new();
    // Direct helper test: bypass HTTP, prove the manifest keys by URL.
    let manifest = generate_from_bytes_for_url(
        "https://image.tmdb.org/t/p/w500/abc.jpg",
        &png,
        staging,
    ).unwrap();
    let entry = manifest.entries.get("https://image.tmdb.org/t/p/w500/abc.jpg");
    assert!(entry.is_some());
    assert!(entry.unwrap().srcset.len() > 0);
}
```
If `make_test_png` doesn't exist, inline a minimal 1×1 PNG byte literal.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p oxibuilder-core media::tests::optimize_external`
Expected: FAIL — `generate_from_bytes_for_url` undefined.

- [ ] **Step 3: Extract a bytes→entry helper + implement optimize_external**

Refactor `generate`'s body so the core (decode → resize → encode → write variants) is reusable. Add:
```rust
/// Build a manifest entry for an already-downloaded byte blob, keyed by `url`.
/// Reuses the SHA-256 cache + `generate()` resize logic. Mirrors `optimize`'s
/// per-ref loop but the key is the external URL, not a `media/...` logical ref.
fn entry_from_bytes(bytes: &[u8], derived: &Path, cache: &mut HashMap<String, Vec<ImageSrc>>) -> Option<ImageEntry> {
    let sha8 = hex8(&Sha256::digest(bytes));
    let key = format!("ext:{sha8}");
    let cached_ok = cache.get(&key)
        .filter(|v| v.iter().all(|s| derived.join(url_file(&s.url)).exists()))
        .cloned();
    match cached_ok {
        Some(srcset) => decode_dims_and_entry(bytes, srcset),
        None => match generate(bytes, &sha8, derived) {
            Ok(e) => { cache.insert(key, e.srcset.clone()); Some(e) }
            Err(e) => { tracing::warn!(error = %e, "media: external generate failed, skipping"); None }
        },
    }
}

/// Download each external URL, WebP-encode, return a manifest keyed by URL.
/// Network/decode failures are skipped (logged), never errored.
pub async fn optimize_external(
    urls: &[String],
    staging_dir: &Path,
    http: &reqwest::Client,
) -> io::Result<ImageManifest> {
    let derived = staging_dir.join("media").join("_derived");
    std::fs::create_dir_all(&derived)?;
    let cache_path = derived.join(".cache.json");
    let mut cache: HashMap<String, Vec<ImageSrc>> = read_cache(&cache_path);
    let mut manifest = ImageManifest::empty();
    for url in urls {
        if !url.starts_with("http://") && !url.starts_with("https://") { continue; }
        let bytes = match http.get(url.as_str()).send().await {
            Ok(r) if r.status().is_success() => match r.bytes().await {
                Ok(b) => b.to_vec(),
                Err(e) => { tracing::warn!(url = %url, error = %e, "external image body failed, skipping"); continue; }
            },
            Ok(r) => { tracing::warn!(url = %url, status = %r.status(), "external image http error, skipping"); continue; }
            Err(e) => { tracing::warn!(url = %url, error = %e, "external image fetch failed, skipping"); continue; }
        };
        if let Some(entry) = entry_from_bytes(&bytes, &derived, &mut cache) {
            manifest.entries.insert(url.clone(), entry);
        }
    }
    write_cache(&cache_path, &cache)?;
    Ok(manifest)
}
```
Make the test call `entry_from_bytes` (or the URL-keyed wrapper) directly to avoid network in the unit test.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p oxibuilder-core media`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/oxibuilder-core/src/media.rs
git commit -m "feat(media): add optimize_external for http(s) image sources"
```

---

### Task 2: `BuildExt::external_image_urls` hook + movies/books impls

**Files:**
- Modify: `crates/oxibuilder-core/src/builder.rs`, `crates/oxibuilder-ext-movies/src/lib.rs`, `crates/oxibuilder-ext-books/src/lib.rs`

**Interfaces:**
- Produces: `BuildExt::external_image_urls(&self, db, rt) -> Result<Vec<String>, Box<dyn Error + Send + Sync>>` (default `Ok(vec![])`).

- [ ] **Step 1: Add the default method to the trait**

In `builder.rs` `BuildExt` (82-110), add after `build_search_docs`:
```rust
/// External http(s) image URLs this extension contributes (posters, covers).
/// Collected at build time for local WebP optimization. Default: none.
fn external_image_urls(
    &self,
    _db: &SqlitePool,
    _rt: &tokio::runtime::Handle,
) -> Result<Vec<String>, Box<dyn Error + Send + Sync>> {
    Ok(Vec::new())
}
```
This default keeps every other implementor (blog/activity/novels/projects/scraps/links/profile) compiling untouched.

- [ ] **Step 2: Implement for movies**

In `ext-movies/src/lib.rs` `impl BuildExt for MoviesExtension` (389+), add `external_image_urls` that runs (via `rt.block_on`) a query selecting `poster_path` from published movie entries, filtering to `http(s)` URLs:
```rust
fn external_image_urls(&self, db: &SqlitePool, rt: &tokio::runtime::Handle) -> Result<Vec<String>, Box<dyn Error + Send + Sync>> {
    let urls = rt.block_on(async {
        let rows: Vec<(Option<String>,)> = sqlx::query_as(
            "SELECT poster_path FROM movie_entry WHERE published_at IS NOT NULL AND poster_path LIKE 'http%'"
        ).fetch_all(db).await
    }).map_err(|e| -> Box<dyn Error + Send + Sync> { e.to_string().into() })?;
    Ok(rows.into_iter().filter_map(|(p,)| p).collect())
}
```
(Adjust the column list if `poster_path` stores the TMDB path `/abc.jpg` rather than a full URL — see Task 4 front-end note: the SPA builds `https://image.tmdb.org/t/p/w200{path}`. If the DB stores only the path, emit the full URL here so the manifest key matches what the SPA resolves. Confirm by reading `MovieEntry.poster_path` usage in `MovieCard.tsx` — it's `poster_path` passed to `posterUrl()` which prepends the TMDB base. So build the full URL in this hook.)

- [ ] **Step 3: Implement for books**

In `ext-books/src/lib.rs` `impl BuildExt for BooksExtension` (223+), add `external_image_urls` selecting `cover_image_url` (already a full URL per the Aladin/Google mapping in `client.rs`) where it starts with `http`.

- [ ] **Step 4: Verify compile across the workspace**

Run: `cargo check --workspace`
Expected: compiles (default impl covers all other extensions).

- [ ] **Step 5: Commit**

```bash
git add crates/oxibuilder-core/src/builder.rs crates/oxibuilder-ext-movies/src/lib.rs crates/oxibuilder-ext-books/src/lib.rs
git commit -m "feat(build): external_image_urls hook on BuildExt + movies/books"
```

---

### Task 3: Merge external images into `run_image_pre_pass`

**Files:**
- Modify: `crates/oxibuilder-core/src/build.rs`, `crates/oxibuilder-cli/src/commands/build.rs`, `crates/oxibuilder-console/src/build/build_run.rs`

**Interfaces:**
- Produces: `run_image_pre_pass(db, media_dir, data_dir, builders, rt)` now also accepts `&[Box<dyn BuildExt>]` + `&Handle`, collects each extension's external URLs, runs `optimize_external`, and merges into the returned manifest.

- [ ] **Step 1: Extend the pre-pass signature + body**

In `build.rs` `run_image_pre_pass` (185-221), change the signature to `run_image_pre_pass(db, media_dir, data_dir, builders, rt)` — adding both `builders: &[Box<dyn BuildExt>]` and `rt: &tokio::runtime::Handle`. The `rt` is required because `external_image_urls` is a sync trait method that internally calls `rt.block_on` for its DB query (`run_image_pre_pass` is async, but the per-builder hook is sync). After the local `refs`/`optimize` block, add:
```rust
// Collect external image URLs from every extension and optimize them too.
let mut external: Vec<String> = Vec::new();
for b in builders {
    match b.external_image_urls(db, rt) {  // rt = a tokio Handle captured in build_site's caller
        Ok(urls) => external.extend(urls),
        Err(e) => tracing::warn!(ext = b.ext_id(), error = %e, "external_image_urls failed, skipping"),
    }
}
let mut manifest = /* existing local manifest or empty */;
if !external.is_empty() {
    let http = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(15))
        .build().unwrap_or_else(|_| reqwest::Client::new());
    let ext_manifest = optimize_external(&external, &staging_dir, &http).await?;
    for (k, v) in ext_manifest.entries { manifest.entries.insert(k, v); }
}
```
Note: `run_image_pre_pass` is `async` and called from a runtime context; capture/derive the `rt: &tokio::runtime::Handle` there (the CLI/console callers already have one). Adjust the staging-dir/manifest merge so an all-external build (no blog refs) still returns `(Some(staging), Some(manifest))` rather than the early `Ok((None, None))`.

- [ ] **Step 2: Update the two callers**

In `crates/oxibuilder-cli/src/commands/build.rs` (43-50) and `crates/oxibuilder-console/src/build/build_run.rs` (84-106): build a `builders` slice (cheap — `all_builders_with_image_manifest(None)`) and pass it to `run_image_pre_pass`. Keep the existing post-pre-pass `all_builders_with_image_manifest(manifest)` call for the build vec (manifest injection into BlogExtension is unaffected).

- [ ] **Step 3: Verify compile + existing build tests**

Run: `cargo test -p oxibuilder-core`
Expected: `ssg_build.rs` still passes (its `StubBuilder`/`BlogShellBuilder` use the default `external_image_urls` → empty). Adjust call sites in those tests if they invoke `run_image_pre_pass` directly.

- [ ] **Step 4: Commit**

```bash
git add crates/oxibuilder-core/src/build.rs crates/oxibuilder-cli/src/commands/build.rs crates/oxibuilder-console/src/build/build_run.rs
git commit -m "feat(build): merge external image optimization into pre-pass"
```

---

### Task 4: Front-end — resolve external URLs via manifest

**Files:**
- Modify: `web/src/shared/image-manifest.ts`
- Create: `web/src/shared/useOptimizedImage.ts`
- Modify: `web/src/extensions/movies/MovieCard.tsx`, `MovieDetailPage.tsx`, `web/src/extensions/books/BookCard.tsx`

**Interfaces:**
- Produces: `isOptimizableRef(src)` true for `media/...` OR `https?://`; `resolveMedia` works for any key; `useOptimizedImage(url)` returns `{ src, srcset, width, height } | null`.

- [ ] **Step 1: Broaden the ref predicate**

In `image-manifest.ts`, rename/replace `isMediaRef`:
```ts
/** True for `media/...` logical refs OR external http(s) URLs. */
export function isOptimizableRef(src: string): boolean {
  const s = src.trim();
  return /^\/?media\//.test(s) || /^https?:\/\//.test(s);
}
```
Keep `isMediaRef` as a re-export alias (markdown rule still calls it) to avoid churn, or update the call site. `resolveMedia(src, base, m)` already does `m[src] ?? null`; no key-namespace change needed since external URLs are keys verbatim.

- [ ] **Step 2: Add the optimized-image hook**

Create `web/src/shared/useOptimizedImage.ts`:
```ts
import { useSyncExternalStore } from "react";
import { getImageManifest, type ManifestEntry, type ManifestSrc } from "./image-manifest";

/** Resolve an image URL (media ref or http(s)) to its optimized srcset/dims,
 *  or null when no manifest entry exists (live/preview fallback). */
export function useOptimizedImage(src: string | null | undefined): ManifestEntry | null {
  const m = useSyncExternalStore(
    () => () => {},              // no subscribe; manifest loads once at module init
    () => getImageManifest(),    // snapshot
    () => ({}),                  // SSR
  );
  if (!src) return null;
  const key = src.trim();
  return m[key] ?? null;
}

/** Largest variant ≤ 960px, else the largest available. */
export function pickVariant(entry: ManifestEntry): ManifestSrc {
  const sorted = [...entry.srcset].sort((a, b) => a.width - b.width);
  return sorted.find((v) => v.width <= 960) ?? sorted[sorted.length - 1] ?? entry.srcset[0];
}
```

- [ ] **Step 3: Use the hook in MovieCard**

In `MovieCard.tsx`, replace the raw `posterUrl(movie.poster_path)` `<img>` (66-72) with a manifest-aware version:
```tsx
const optimized = useOptimizedImage(img);
const variant = optimized ? pickVariant(optimized) : null;
// ...
{variant ? (
  <img
    src={variant.url}
    srcSet={optimized!.srcset.map((s) => `${s.url} ${s.width}w`).join(", ")}
    sizes="80px"
    width={optimized!.width} height={optimized!.height}
    alt="" loading="lazy" decoding="async"
    className="w-16 shrink-0 rounded-md object-cover sm:w-20"
  />
) : img ? (
  <img src={img} alt="" loading="lazy" className="w-16 shrink-0 rounded-md object-cover sm:w-20" />
) : ( /* existing Film fallback */ )}
```
Important: the manifest key must match what `external_image_urls` emitted. If the hook stored the full TMDB URL (`https://image.tmdb.org/t/p/w200{path}`), `img` here is built the same way — they match. (Track A Task 2 note applies; confirm `posterUrl()` width prefix is consistent with what the build emitted. Pick **one** canonical width, e.g. `w500`, for both the build-time key and the front-end lookup — use `w500` since `MovieDetailPage` already uses 500.)

- [ ] **Step 4: Use the hook in MovieDetailPage + BookCard**

Apply the same pattern. For `MovieDetailPage` (larger poster), `sizes="160px"`. For `BookCard`, resolve `cover_image_url`.

- [ ] **Step 5: Verify front-end builds + types**

Run: `cd web && bun run build` (or typecheck)
Expected: passes.

- [ ] **Step 6: Commit**

```bash
git add web/src/shared/image-manifest.ts web/src/shared/useOptimizedImage.ts web/src/extensions
git commit -m "feat(web): resolve external posters/covers via image manifest"
```

---

### Task 5: Eager/lazy priority for above-the-fold cards

**Files:**
- Modify: `web/src/extensions/movies/MovieCard.tsx`, `web/src/extensions/books/BookCard.tsx`, `web/src/extensions/movies/MoviesPage.tsx`, `web/src/extensions/books/BooksPage.tsx`

**Interfaces:**
- Consumes: the card index in the grid (passed as a prop).

- [ ] **Step 1: Thread an index + EAGER_COUNT**

In `MoviesPage.tsx` grid (~322-352), pass `index={i}` to `<MovieCard>`. Define `const EAGER_COUNT = 10;`. In `MovieCard`, accept `index?: number` and set `loading={index != null && index < EAGER_COUNT ? "eager" : "lazy"}` and `fetchPriority={index != null && index < EAGER_COUNT ? "high" : "auto"}` on the `<img>`.

- [ ] **Step 2: Same for books**

Apply the identical pattern in `BooksPage`/`BookCard`.

- [ ] **Step 3: Verify build**

Run: `cd web && bun run build`
Expected: passes.

- [ ] **Step 4: Commit**

```bash
git add web/src/extensions
git commit -m "perf(web): eager-load above-the-fold poster/cover images"
```

---

### Task 6: Build smoke + verification

**Files:**
- Modify: `crates/oxibuilder-core/tests/ssg_build.rs` (extend the external-image path)

- [ ] **Step 1: Add an external-image build test**

In `ssg_build.rs`, add a `StubBuilder` variant whose `external_image_urls` returns a `data:`/local-file URL (or a tiny mock server), run the pre-pass, and assert the manifest carries the external-URL key and `out/media/_derived/*.webp` exists. Guard network with a local fixture to keep the test deterministic.

- [ ] **Step 2: Run the full suite**

Run: `cargo test` and `cd web && bun test`
Expected: all pass.

- [ ] **Step 3: Smoke-build a real site**

Run: `cargo build --release && ./target/release/oxibuilder build` against a site with ≥1 movie poster.
Expected: build completes; `out/data/image-manifest.json` contains the TMDB URL key; `out/media/_derived/` has the WebP variants.

- [ ] **Step 4: Commit**

```bash
git add crates/oxibuilder-core/tests/ssg_build.rs
git commit -m "test: external image build-time optimization smoke"
```
