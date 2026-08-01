# Inline Media Authoring Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let the owner insert images/GIFs into markdown content bodies and have them render correctly in live Admin, draft preview, built preview, and the deployed public site — by extending the existing per-site media module.

**Architecture:** Three gaps to close: (G1) markdown renderers ignore asset resolvers → make `markdown-it`/`marked` resolve `media/...` image srcs via a shared `AssetResolverContext`; (G2) no editor insertion → add a toolbar + drag/drop/paste + `MediaPicker` to `MarkdownEditor`; (G3) no media library → add list + delete endpoints and a picker UI. Build-path correctness (G4) holds via `<base>` + `publicAssetResolver`, locked by a test.

**Tech Stack:** Rust/axum (backend), React 19 + TypeScript + Vite (web), `markdown-it@14`, `marked@18`, `@tanstack/react-query`, Radix dialog, Tailwind v4.

## Global Constraints

- Media store path is fixed: `media/<extension>/<uuid>.<ext>` under the site `media_dir`. Never invent new storage.
- Supported image formats: JPEG/PNG/WebP/GIF only (magic-byte validated). SVG excluded.
- 10 MiB per-file cap (enforced streaming in `upload.rs`).
- `media/...` is the canonical logical reference stored in content rows; never store a leading slash.
- Path-containment: reuse the component-normal + canonicalize-under-`media_dir` checks from `serve.rs` for every new endpoint.
- 1-person-owner security model: delete is manual + confirmation; no reference tracking this iteration.
- One resolver from context into both markdown renderers; NO server-side markdown rewriting; NO preview/public branching — `publicAssetResolver()` (keys off `document.baseURI`) serves built site AND live preview. Only the Admin `EditorPreviewDrawer` threads `adminAssetResolver(slug)`.
- Conventions: Rust `cargo fmt`/`clippy` clean; `npx tsc --noEmit` clean; commit after each task.

---

### Task 1: Media list + delete endpoints (backend)

**Files:**
- Create: `crates/oxipage-console/src/media/library.rs`
- Modify: `crates/oxipage-console/src/media/mod.rs` (wire routes)
- Test: `crates/oxipage-console/tests/media.rs`

**Interfaces:**
- Consumes: `Extension<Arc<SiteContext>>` (same as `serve.rs`), `ctx.media_dir`, `ctx.media_dir.join(extension)`.
- Produces: `GET /api/console/s/{slug}/media[?extension=X]` → `{ data: MediaItem[] }`; `DELETE /api/console/s/{slug}/media/{extension}/{file}` → 204.

`MediaItem` JSON shape (must match the frontend client in Task 3):
```json
{ "path": "media/blog/<uuid>.webp", "extension": "blog", "file": "<uuid>.webp",
  "mime": "image/webp", "bytes": 42133, "updated_at": "2026-08-01T00:00:00.000Z" }
```

- [ ] **Step 1: Write failing tests for list + delete**

Append to `crates/oxipage-console/tests/media.rs`:
```rust
#[tokio::test]
async fn list_media_after_upload() {
    let (dir, app) = build_app().await;
    // upload one PNG into the "blog" extension (reuse build_multipart + PNG_1X1)
    let (boundary, body) = build_multipart("a.png", PNG_1X1);
    let upload = app
        .clone()
        .oneshot(
            Request::post("/api/console/s/blog/media/blog")
                .header("content-type", format!("multipart/form-data; boundary={boundary}"))
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(upload.status(), StatusCode::OK);
    let up: serde_json::Value =
        serde_json::from_slice(&hyper::body::to_bytes(upload.into_body(), usize::MAX).await.unwrap())
            .unwrap();
    let path = up["data"]["path"].as_str().unwrap().to_string();

    let res = app
        .oneshot(Request::get("/api/console/s/blog/media").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let list: serde_json::Value =
        serde_json::from_slice(&to_bytes(res.into_body(), usize::MAX).await.unwrap()).unwrap();
    let items = list["data"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["path"], path);
    assert_eq!(items[0]["extension"], "blog");
    assert!(items[0]["bytes"].as_u64().unwrap() > 0);

    // extension filter excludes other namespaces
    let res2 = app
        .oneshot(Request::get("/api/console/s/blog/media?extension=profile").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let list2: serde_json::Value =
        serde_json::from_slice(&to_bytes(res2.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert!(list2["data"].as_array().unwrap().is_empty());
    let _ = dir; // keep temp dir alive
}

#[tokio::test]
async fn delete_media_removes_file() {
    let (_dir, app) = build_app().await;
    let (boundary, body) = build_multipart("a.png", PNG_1X1);
    let upload = app
        .clone()
        .oneshot(
            Request::post("/api/console/s/blog/media/blog")
                .header("content-type", format!("multipart/form-data; boundary={boundary}"))
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    let up: serde_json::Value =
        serde_json::from_slice(&to_bytes(upload.into_body(), usize::MAX).await.unwrap()).unwrap();
    let path = up["data"]["path"].as_str().unwrap().to_string(); // media/blog/<uuid>.png
    let file = path.rsplit('/').next().unwrap().to_string();

    let res = app
        .clone()
        .oneshot(
            Request::delete(format!("/api/console/s/blog/media/blog/{file}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    // serve now 404
    let res2 = app
        .oneshot(Request::get(format!("/api/console/s/blog/media/blog/{file}")).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res2.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn delete_rejects_traversal() {
    let (_dir, app) = build_app().await;
    let res = app
        .oneshot(Request::delete("/api/console/s/blog/media/blog/..%2F..%2Fetc%2Fpasswd").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert!(res.status() == StatusCode::BAD_REQUEST || res.status() == StatusCode::NOT_FOUND);
}
```
Add imports at top if missing: `use axum::body::to_bytes;` (already used elsewhere in file per existing tests — confirm).

- [ ] **Step 2: Run tests to verify they fail (404 / no route)**

Run: `cargo test -p oxipage-console --test media`
Expected: the new tests FAIL (routes don't exist yet → 404 on list, 404 on delete).

- [ ] **Step 3: Implement `library.rs`**

Create `crates/oxipage-console/src/media/library.rs`:
```rust
//! Media library: enumerate + delete uploaded media (spec §5.3).

use crate::sites_runtime::SiteContext;
use axum::Extension;
use axum::Json;
use axum::extract::{Path, Query};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Serialize;
use std::path::{Component, PathBuf};
use std::sync::Arc;

#[derive(Serialize)]
pub struct MediaItem {
    pub path: String,
    pub extension: String,
    pub file: String,
    pub mime: String,
    pub bytes: u64,
    pub updated_at: String,
}

#[derive(Serialize)]
pub struct ListResponse {
    pub data: Vec<MediaItem>,
}

#[derive(serde::Deserialize, Default)]
pub struct ListQuery {
    pub extension: Option<String>,
}

/// Validate a single path segment the same way serve.rs does.
fn safe_segment(seg: &str) -> Option<String> {
    let s = seg.trim();
    if s.is_empty() || s == "." || s == ".." {
        return None;
    }
    for component in std::path::Path::new(s).components() {
        match component {
            Component::Normal(p) => {
                if p.is_empty() {
                    return None;
                }
            }
            _ => return None,
        }
    }
    Some(s.to_string())
}

pub async fn list_handler(
    Extension(ctx): Extension<Arc<SiteContext>>,
    Query(q): Query<ListQuery>,
) -> Result<Json<ListResponse>, StatusCode> {
    let mut items = Vec::new();
    let canonical_media = ctx
        .media_dir
        .canonicalize()
        .unwrap_or_else(|_| ctx.media_dir.clone());

    let extension_dirs: Vec<PathBuf> = match &q.extension {
        Some(ext) => {
            let Some(ext) = safe_segment(ext) else {
                return Err(StatusCode::BAD_REQUEST);
            };
            vec![ctx.media_dir.join(ext)]
        }
        None => match tokio::fs::read_dir(&ctx.media_dir).await {
            Ok(mut rd) => {
                let mut v = Vec::new();
                while let Ok(Some(entry)) = rd.next_entry().await {
                    if entry.path().is_dir() {
                        v.push(entry.path());
                    }
                }
                v
            }
            Err(_) => return Ok(Json(ListResponse { data: items })),
        },
    };

    for ext_dir in extension_dirs {
        let extension = ext_dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let mut rd = match tokio::fs::read_dir(&ext_dir).await {
            Ok(rd) => rd,
            Err(_) => continue,
        };
        while let Ok(Some(entry)) = rd.next_entry().await {
            let path = entry.path();
            let meta = match tokio::fs::metadata(&path).await {
                Ok(m) if m.is_file() => m,
                _ => continue,
            };
            // containment: canonicalize must stay under media_dir
            let canon = path.canonicalize().unwrap_or_else(|_| path.clone());
            if !canon.starts_with(&canonical_media) {
                continue;
            }
            let file = match path.file_name() {
                Some(n) => n.to_string_lossy().to_string(),
                None => continue,
            };
            let mime = mime_guess::from_path(&path)
                .first_or_octet_stream()
                .to_string();
            let updated_at = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| {
                    // RFC3339-ish UTC; chrono-free via humantime-free manual is messy,
                    // use milliseconds-since-epoch as ISO if chrono unavailable.
                    format_unix_ms(d.as_millis() as i64)
                })
                .unwrap_or_default();
            items.push(MediaItem {
                path: format!("media/{extension}/{file}"),
                extension: extension.clone(),
                file,
                mime,
                bytes: meta.len(),
                updated_at,
            });
        }
    }
    items.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(Json(ListResponse { data: items }))
}

/// RFC3339 UTC from unix milliseconds (no chrono dependency).
fn format_unix_ms(ms: i64) -> String {
    let secs = ms / 1000;
    let days = secs / 86400;
    let rem = secs % 86400;
    let h = rem / 3600;
    let mi = (rem % 3600) / 60;
    let s = rem % 60;
    let (y, mo, d) = civil_from_days(days);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}.000Z")
}

/// Howard Hinnant's days-from-civil inverse. Input = days since 1970-01-01.
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

pub async fn delete_handler(
    Extension(ctx): Extension<Arc<SiteContext>>,
    Path((extension, file)): Path<(String, String)>,
) -> Result<impl IntoResponse, StatusCode> {
    let mut candidate = PathBuf::from(&ctx.media_dir);
    for seg in [&extension, &file] {
        let Some(seg) = safe_segment(seg) else {
            return Err(StatusCode::BAD_REQUEST);
        };
        candidate.push(seg);
    }
    let canonical_media = ctx
        .media_dir
        .canonicalize()
        .unwrap_or_else(|_| ctx.media_dir.clone());
    let canonical_candidate = candidate
        .canonicalize()
        .unwrap_or_else(|_| candidate.clone());
    if !canonical_candidate.starts_with(&canonical_media) {
        return Err(StatusCode::BAD_REQUEST);
    }
    match tokio::fs::remove_file(&candidate).await {
        Ok(_) => Ok(StatusCode::NO_CONTENT),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}
```

- [ ] **Step 4: Wire routes in `media/mod.rs`**

In `crates/oxipage-console/src/media/mod.rs`, add `pub mod library;` and extend `router()`:
```rust
pub fn router() -> Router {
    Router::new()
        .route("/media", get(library::list_handler))
        .route("/media/{extension}", post(upload::upload_handler).layer(DefaultBodyLimit::max(12 * 1024 * 1024)))
        .route(
            "/media/{extension}/{file}",
            get(serve::serve_handler)
                .head(serve::serve_handler)
                .delete(library::delete_handler),
        )
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p oxipage-console --test media`
Expected: PASS (list, delete, traversal, plus existing upload/serve tests).

- [ ] **Step 6: Commit**

```bash
git add crates/oxipage-console/src/media/library.rs crates/oxipage-console/src/media/mod.rs crates/oxipage-console/tests/media.rs
git commit -m "feat(console): media library list + delete endpoints"
```

---

### Task 2: Shared AssetResolverContext + resolver-aware Markdown

**Files:**
- Create: `web/src/shared/asset-context.tsx`
- Modify: `web/src/shared/Markdown.tsx`
- Modify: `web/src/admin/shared/ui/EditorPreviewDrawer.tsx` (re-export from shared, delete in-file copy)
- Modify: `web/src/App.tsx` (wrap public tree)

**Interfaces:**
- Consumes: `publicAssetResolver`, `adminAssetResolver`, `type AssetResolver` from `shared/assets.ts`.
- Produces: `AssetResolverProvider`, `useAssetResolver`, `PublicThemeScope` from `shared/asset-context.tsx`. `Markdown` resolves `media/...` image srcs.

- [ ] **Step 1: Create `shared/asset-context.tsx`**

```tsx
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
 *  correct context. "public" = built site + live public preview (document.baseURI);
 *  "admin" = the Admin SPA (server media endpoint). */
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

/** Consumer hook. Default publicAssetResolver when no provider is mounted
 *  (safe for the public SPA, which renders at the deployment base). */
export function useAssetResolver(): AssetResolver {
  return useContext(AssetResolverContext) ?? publicAssetResolver();
}
```

- [ ] **Step 2: Make `Markdown.tsx` resolver-aware**

Replace `web/src/shared/Markdown.tsx`:
```tsx
import { useMemo } from "react";
import MarkdownIt from "markdown-it";
import { useAssetResolver } from "./asset-context";

/** True for `media/...` logical references (with or without a leading slash). */
function isMediaRef(src: string): boolean {
  return /^\/?media\//.test(src.trim());
}

export function Markdown({ source }: { source: string }) {
  const resolver = useAssetResolver();
  const html = useMemo(() => {
    const md = new MarkdownIt({ linkify: true });
    const renderImage =
      md.renderer.rules.image ??
      ((tokens, idx, options, _env, self) => self.renderToken(tokens, idx, options));
    md.renderer.rules.image = (tokens, idx, opts, env, self) => {
      const src = tokens[idx].attrGet("src") ?? "";
      if (isMediaRef(src)) {
        const resolved = resolver.resolve(src);
        if (resolved) tokens[idx].attrSet("src", resolved);
      }
      return renderImage(tokens, idx, opts, env, self);
    };
    return md.render(source);
  }, [source, resolver]);
  // 서버에 저장된 오너 본인의 마크다운이라 sanitize 없이 렌더링한다 (1인 오너 전제, doc §0.3).
  return <div className="markdown" dangerouslySetInnerHTML={{ __html: html }} />;
}
```

- [ ] **Step 3: Re-export from shared in `EditorPreviewDrawer.tsx`**

In `web/src/admin/shared/ui/EditorPreviewDrawer.tsx`: delete the in-file `createContext`/`AssetResolverContext`/`AssetResolverProvider`/`useAssetResolver` block (lines ~15-58) and the `import { createContext, useContext } from "react"`. Keep `PublicThemeScope` in place (or also move — see step). Replace with:
```tsx
import {
  AssetResolverProvider,
  PublicThemeScope,
} from "../../../shared/asset-context";
```
Move `PublicThemeScope` into `shared/asset-context.tsx` too (add it there verbatim) so all the resolver+theme plumbing lives in one shared module, and remove the in-file copy. Keep the `<AssetResolverProvider mode="admin" slug={slug}>` usage at the preview site unchanged (the `mode`/`slug` props are identical).

- [ ] **Step 4: Wrap the public SPA tree in `App.tsx`**

In `web/src/App.tsx`, import `AssetResolverProvider` and wrap `<BrowserRouter>` (or `<Routes>`) so the public tree resolves against `document.baseURI`:
```tsx
import { AssetResolverProvider } from "./shared/asset-context";
// ...
export function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <AssetResolverProvider mode="public">
        <BrowserRouter>
          <Routes>
            <Route path="/*" element={<Shell />} />
          </Routes>
        </BrowserRouter>
      </AssetResolverProvider>
    </QueryClientProvider>
  );
}
```

- [ ] **Step 5: Typecheck**

Run: `cd web && npx tsc --noEmit`
Expected: clean (no errors). Fix any dangling imports of the old in-file exports.

- [ ] **Step 6: Commit**

```bash
git add web/src/shared/asset-context.tsx web/src/shared/Markdown.tsx web/src/admin/shared/ui/EditorPreviewDrawer.tsx web/src/App.tsx
git commit -m "feat(web): resolver-aware Markdown via shared AssetResolverContext"
```

---

### Task 3: Media library API clients + MediaPicker

**Files:**
- Modify: `web/src/admin/shared/api.ts`
- Create: `web/src/admin/shared/ui/MediaPicker.tsx`

**Interfaces:**
- Consumes: `GET /api/console/s/{slug}/media[?extension=X]`, `DELETE /api/console/s/{slug}/media/{ext}/{file}`, `uploadImage`, `adminAssetResolver`, Radix `Dialog`.
- Produces: `listMedia(slug, extension?)`, `deleteMedia(slug, extension, file)`, `<MediaPicker slug extension? onPick trigger />`.

- [ ] **Step 1: Add API clients**

Append to `web/src/admin/shared/api.ts` (after `uploadImage`):
```ts
export interface MediaItem {
  path: string;
  extension: string;
  file: string;
  mime: string;
  bytes: number;
  updated_at: string;
}

/** List the site's uploaded media. Optional extension namespace filter. */
export async function listMedia(
  slug: string,
  extension?: string,
): Promise<MediaItem[]> {
  const qs = extension ? `?extension=${encodeURIComponent(extension)}` : "";
  const res = await siteScopedFetch(slug, `/media${qs}`);
  const body = await jsonOrThrow<{ data: MediaItem[] }>(res);
  return body.data;
}

/** Delete one uploaded media file. Returns true on 204. */
export async function deleteMedia(
  slug: string,
  extension: string,
  file: string,
): Promise<void> {
  const res = await siteScopedFetch(
    slug,
    `/media/${encodeURIComponent(extension)}/${encodeURIComponent(file)}`,
    { method: "DELETE" },
  );
  if (!res.ok && res.status !== 204) {
    throw new Error(`Delete failed (${res.status})`);
  }
}
```

- [ ] **Step 2: Create `MediaPicker.tsx`**

Create `web/src/admin/shared/ui/MediaPicker.tsx`:
```tsx
import { useState } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import * as Dialog from "@radix-ui/react-dialog";
import { ImagePlus, Trash2, Upload } from "lucide-react";
import { adminAssetResolver } from "../../../shared/assets";
import {
  listMedia,
  deleteMedia,
  uploadImage,
  type MediaItem,
} from "../api";
import { Button } from "../../../shared/ui/button";

interface MediaPickerProps {
  slug: string;
  /** Namespace to filter by; undefined shows all extensions. */
  extension?: string;
  /** Called with the chosen logical `media/...` path. */
  onPick: (path: string) => void;
  trigger: React.ReactNode;
}

export function MediaPicker({ slug, extension, onPick, trigger }: MediaPickerProps) {
  const qc = useQueryClient();
  const [open, setOpen] = useState(false);
  const [ext, setExt] = useState<string | undefined>(extension);
  const resolver = adminAssetResolver(slug);
  const key = ["media", slug, ext];

  const { data, isLoading } = useQuery({
    queryKey: key,
    queryFn: () => listMedia(slug, ext),
    enabled: open,
  });

  const del = useMutation({
    mutationFn: (item: MediaItem) => deleteMedia(slug, item.extension, item.file),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["media", slug] }),
  });

  const upload = useMutation({
    mutationFn: (file: File) => uploadImage(slug, ext ?? "blog", file),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["media", slug] }),
  });

  function onUpload(e: React.ChangeEvent<HTMLInputElement>) {
    const f = e.target.files?.[0];
    if (f) upload.mutate(f);
    e.target.value = "";
  }

  function pick(path: string) {
    onPick(path);
    setOpen(false);
  }

  return (
    <Dialog.Root open={open} onOpenChange={setOpen}>
      <Dialog.Trigger asChild>{trigger}</Dialog.Trigger>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 bg-black/40 z-40" />
        <Dialog.Content className="fixed left-1/2 top-1/2 z-50 -translate-x-1/2 -translate-y-1/2 w-[640px] max-w-[92vw] max-h-[80vh] overflow-auto rounded-lg border border-line bg-canvas p-4 shadow-lg">
          <Dialog.Title className="text-base font-semibold mb-3">Media library</Dialog.Title>
          <div className="flex items-center gap-2 mb-3">
            <select
              className="border border-line rounded px-2 py-1 text-sm bg-surface"
              value={ext ?? ""}
              onChange={(e) => setExt(e.target.value || undefined)}
            >
              <option value="">All</option>
              <option value="blog">blog</option>
              <option value="projects">projects</option>
              <option value="profile">profile</option>
              <option value="novels">novels</option>
              <option value="books">books</option>
              <option value="movies">movies</option>
              <option value="scraps">scraps</option>
            </select>
            <label className="inline-flex items-center gap-1 text-sm cursor-pointer text-primary">
              <Upload className="size-4" />
              <span>Upload</span>
              <input type="file" accept="image/png,image/jpeg,image/webp,image/gif" className="hidden" onChange={onUpload} />
            </label>
            {upload.isPending && <span className="text-xs text-muted">Uploading…</span>}
            {upload.isError && <span className="text-xs text-red-600">Upload failed</span>}
          </div>
          {isLoading ? (
            <p className="text-sm text-muted">Loading…</p>
          ) : !data || data.length === 0 ? (
            <p className="text-sm text-muted">No media yet. Upload an image.</p>
          ) : (
            <div className="grid grid-cols-3 sm:grid-cols-4 gap-3">
              {data.map((item) => (
                <div key={item.path} className="group relative border border-line rounded overflow-hidden">
                  <button
                    type="button"
                    className="block w-full aspect-square bg-surface"
                    onClick={() => pick(item.path)}
                    title="Insert"
                  >
                    <img src={resolver.resolve(item.path) ?? ""} alt={item.file} className="w-full h-full object-cover" />
                  </button>
                  <button
                    type="button"
                    className="absolute top-1 right-1 rounded bg-black/50 p-1 text-white opacity-0 group-hover:opacity-100"
                    title="Delete"
                    onClick={() => {
                      if (confirm("Delete this asset? It may be referenced in content.")) del.mutate(item);
                    }}
                  >
                    <Trash2 className="size-3.5" />
                  </button>
                </div>
              ))}
            </div>
          )}
          <div className="flex justify-end mt-4">
            <Dialog.Close asChild>
              <Button variant="outline" size="sm">Close</Button>
            </Dialog.Close>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
```
(If `Button` lacks a `variant`/`size` API used here, match the existing `Button` signature from `shared/ui/button.tsx`.)

- [ ] **Step 3: Typecheck**

Run: `cd web && npx tsc --noEmit`
Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add web/src/admin/shared/api.ts web/src/admin/shared/ui/MediaPicker.tsx
git commit -m "feat(web): media library API clients + MediaPicker"
```

---

### Task 4: MarkdownEditor toolbar + insertion

**Files:**
- Modify: `web/src/admin/shared/ui/MarkdownEditor.tsx`

**Interfaces:**
- Consumes: `uploadImage`, `adminAssetResolver`, `MediaPicker`, `marked`.
- Produces: `<MarkdownEditor value onChange slug extension rows? placeholder? />` — toolbar with image picker; drag-drop + paste upload; resolver-aware preview.

- [ ] **Step 1: Rewrite `MarkdownEditor.tsx`**

Replace `web/src/admin/shared/ui/MarkdownEditor.tsx`:
```tsx
import { useMemo, useRef, useState } from "react";
import { marked } from "marked";
import { ImagePlus } from "lucide-react";
import { Textarea } from "../../../shared/ui/textarea";
import { Button } from "../../../shared/ui/button";
import { adminAssetResolver } from "../../../shared/assets";
import { uploadImage } from "../api";
import { MediaPicker } from "./MediaPicker";

interface Props {
  value: string;
  onChange: (v: string) => void;
  /** Site slug — required for upload + resolver. */
  slug: string;
  /** Extension namespace for uploaded media (e.g. "blog"). */
  extension: string;
  rows?: number;
  placeholder?: string;
}

function isMediaRef(src: string): boolean {
  return /^\/?media\//.test(src.trim());
}

export function MarkdownEditor({
  value,
  onChange,
  slug,
  extension,
  rows = 6,
  placeholder,
}: Props) {
  const [mode, setMode] = useState<"edit" | "preview">("edit");
  const [pending, setPending] = useState(false);
  const [uploadError, setUploadError] = useState<string | null>(null);
  const taRef = useRef<HTMLTextAreaElement>(null);
  const resolver = adminAssetResolver(slug);

  const html = useMemo(() => {
    const r = new marked.Marked({ gfm: true, breaks: false });
    r.use({
      renderer: {
        image({ href, title, text }) {
          const src = String(href ?? "");
          const resolved = isMediaRef(src) ? resolver.resolve(src) ?? src : src;
          const alt = String(text ?? "").replace(/"/g, "&quot;");
          const t = title ? ` title="${String(title).replace(/"/g, "&quot;")}"` : "";
          return `<img src="${resolved}" alt="${alt}"${t} />`;
        },
      },
    });
    return r.parse(value || "") as string;
  }, [value, resolver]);

  /** Splice `text` into the textarea at the current selection, then onChange. */
  function splice(text: string) {
    const ta = taRef.current;
    if (!ta) {
      onChange(value + text);
      return;
    }
    const start = ta.selectionStart ?? value.length;
    const end = ta.selectionEnd ?? value.length;
    const next = value.slice(0, start) + text + value.slice(end);
    onChange(next);
    // restore caret after the inserted text on next tick
    const pos = start + text.length;
    requestAnimationFrame(() => {
      ta.focus();
      ta.setSelectionRange(pos, pos);
    });
  }

  async function uploadAndInsert(file: File) {
    setUploadError(null);
    if (!file.type.startsWith("image/")) {
      setUploadError("Only image files are supported.");
      return;
    }
    setPending(true);
    try {
      const media = await uploadImage(slug, extension, file);
      const alt = file.name.replace(/\.[^.]+$/, "");
      splice(`![${alt}](${media.path})\n`);
    } catch (err) {
      setUploadError(err instanceof Error ? err.message : "Upload failed");
    } finally {
      setPending(false);
    }
  }

  function onDrop(e: React.DragEvent) {
    const f = e.dataTransfer.files?.[0];
    if (f) {
      e.preventDefault();
      uploadAndInsert(f);
    }
  }

  function onPaste(e: React.ClipboardEvent) {
    const f = e.clipboardData.files?.[0];
    if (f) {
      e.preventDefault();
      uploadAndInsert(f);
    }
  }

  return (
    <div className="border border-line rounded overflow-hidden">
      <div className="flex items-center gap-0 border-b border-line bg-surface/30">
        <button
          onClick={() => setMode("edit")}
          className={`px-3 py-1 text-xs font-medium ${mode === "edit" ? "bg-canvas text-foreground" : "text-muted hover:text-foreground"}`}
        >
          Edit
        </button>
        <button
          onClick={() => setMode("preview")}
          className={`px-3 py-1 text-xs font-medium ${mode === "preview" ? "bg-canvas text-foreground" : "text-muted hover:text-foreground"}`}
        >
          Preview
        </button>
        <div className="ml-auto px-2">
          <MediaPicker
            slug={slug}
            extension={extension}
            onPick={(path) => splice(`![](${path})\n`)}
            trigger={
              <Button type="button" variant="ghost" size="sm" className="h-7 gap-1 px-2 text-xs">
                <ImagePlus className="size-3.5" />
                Image
              </Button>
            }
          />
        </div>
      </div>
      {mode === "edit" ? (
        <Textarea
          ref={taRef}
          value={value}
          onChange={(e) => onChange(e.target.value)}
          onDrop={onDrop}
          onPaste={onPaste}
          rows={rows}
          placeholder={placeholder}
          className="border-0 rounded-none"
        />
      ) : (
        <div
          className="p-3 text-sm prose prose-sm max-w-none"
          dangerouslySetInnerHTML={{ __html: html }}
        />
      )}
      {(pending || uploadError) && (
        <div className="px-3 py-1 text-xs border-t border-line">
          {pending && <span className="text-muted">Uploading…</span>}
          {uploadError && <span className="text-red-600">{uploadError}</span>}
        </div>
      )}
    </div>
  );
}
```
NOTE: confirm `Textarea` forwards `ref` and accepts arbitrary handlers (`onDrop`/`onPaste`). If it is a plain wrapper around `<textarea>`, it already does. Confirm `marked@18` renderer API: v18 uses object-form renderer with `{ href, title, text }` tokens — if the installed version differs, adjust to the function-signature form `image(href, title, text)`. Check during implementation.

- [ ] **Step 2: Typecheck + resolve any `marked`/`Textarea` ref API mismatches**

Run: `cd web && npx tsc --noEmit`
Expected: clean. If `marked` renderer signature errors, consult the installed `marked` types and adapt (the contract — rewrite `media/...` src — is unchanged).

- [ ] **Step 3: Commit**

```bash
git add web/src/admin/shared/ui/MarkdownEditor.tsx
git commit -m "feat(web): MarkdownEditor image toolbar + drag/paste/picker insertion"
```

---

### Task 5: Wire MarkdownEditor into content tabs

**Files:**
- Modify: `web/src/admin/content/BlogTab.tsx`, `NovelsTab.tsx`, `ProfileTab.tsx`, `BooksTab.tsx`, `MoviesTab.tsx`, `ScrapsTab.tsx`
- Audit + Modify: `web/src/admin/content/ProjectsTab.tsx`

**Interfaces:**
- Consumes: `<MarkdownEditor slug extension ... />` from Task 4; each tab already receives `slug` as a prop (`{ slug }: { slug: string }`).

- [ ] **Step 1: Pass `slug` + `extension` to every `<MarkdownEditor>`**

For each tab, add `slug={slug}` and the matching `extension` to every `<MarkdownEditor ...>`:
- BlogTab — `extension="blog"`
- NovelsTab — `extension="novels"`
- ProfileTab — `extension="profile"`
- BooksTab — `extension="books"`
- MoviesTab — `extension="movies"`
- ScrapsTab — `extension="scraps"`

Each tab's component signature is `({ slug }: { slug: string })`, so `slug` is in scope.

- [ ] **Step 2: Audit ProjectsTab description editor**

Open `web/src/admin/content/ProjectsTab.tsx`. If the `description_ko`/`description_en` fields use a plain `<Textarea>`, swap each for `<MarkdownEditor slug={slug} extension="projects" ... />` so project descriptions gain the same insertion affordance. If it already uses `MarkdownEditor`, just add the two props. (Confirm the field names from the form state.)

- [ ] **Step 3: Typecheck**

Run: `cd web && npx tsc --noEmit`
Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add web/src/admin/content/
git commit -m "feat(web): wire MarkdownEditor slug+extension into content tabs"
```

---

### Task 6: Build + verify + browser smoke

**Files:** none (verification only)

- [ ] **Step 1: Full Rust build + console tests**

Run: `cargo build --workspace && cargo test -p oxipage-console`
Expected: build green; all console tests pass (existing + new list/delete).

- [ ] **Step 2: Rebuild embedded SPA**

Run: `cd web && bun run build`
Expected: success. Then rebuild the Rust console so it re-embeds the fresh SPA:
Run: `cargo build -p oxipage-console`
(This picks up the new `embedded-spa/` assets via `build.rs`.)

- [ ] **Step 3: tsc clean**

Run: `cd web && npx tsc --noEmit`
Expected: clean.

- [ ] **Step 4: Browser smoke — insert + preview + build correctness**

Start the console: `cargo run -p oxipage -- console` (or the project's run command). In a browser:
1. Open Admin → Blog → edit a post → drag a GIF into the body. Confirm `![name](media/blog/<uuid>.gif)` is spliced and the editor Preview shows it.
2. Save, open the draft/public preview — confirm the GIF renders on `/blog/<slug>/` (nested route) via the public resolver.
3. Open the MediaPicker from the toolbar — confirm it lists the uploaded GIF, the filter works, delete removes it.
4. Run `oxipage build` and open the built preview — confirm the GIF renders on the nested detail route (this is the G4 correctness check).

- [ ] **Step 5: Final commit if any build artifacts regenerated**

```bash
git add -A
git commit -m "chore: rebuild embedded SPA for inline media" || echo "nothing to commit"
```

---

## Self-Review

**Spec coverage:**
- §5.1 shared context → Task 2 ✓
- §5.2 resolver-aware Markdown (both libs) → Task 2 (markdown-it) + Task 4 (marked) ✓
- §5.3 list+delete backend → Task 1 ✓
- §5.4 MediaPicker → Task 3 ✓
- §5.5 editor insertion (toolbar/drag/paste/picker) → Task 4 ✓
- §5.6 callsite updates (6 tabs + projects) → Task 5 ✓
- §5.7 build integration → no change (Task 6 verifies) ✓
- §8 testing → Task 1 (rust), Task 6 (tsc/build/browser) ✓
- §11 acceptance → covered by Tasks 1–6 ✓

**Placeholder scan:** none; ProjectsTab has an explicit audit step with a concrete fallback action.

**Type consistency:** `MediaItem` field names (`path`, `extension`, `file`, `mime`, `bytes`, `updated_at`) match between `library.rs`, the API client, and `MediaPicker`. `listMedia`/`deleteMedia`/`uploadImage` signatures consistent. `MarkdownEditor` prop names (`slug`, `extension`) consistent across Tasks 4–5.
