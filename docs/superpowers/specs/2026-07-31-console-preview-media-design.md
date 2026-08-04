# Console Built Preview and Media — Design Spec

> **Date:** 2026-07-31
> **Subproject:** 3 of 5
> **Predecessor:** Console runtime and routing foundation
> **Consumer:** Extension authoring UX; GitHub Pages deploy

## 1. Goal

Give the Deploy page a faithful preview of the last static build and give Admin forms a safe image-upload path that works immediately, in draft preview, in built preview, and after deployment.

## 2. Current state

- `GET /api/console/preview/{slug}/{*rest}` reads one exact file under `<site>/out`.
- It has no directory-index resolution and no SPA fallback.
- It does not rewrite public base paths.
- Static HTML currently references path-absolute `/assets/...`; public static data fetches use `/data/...`. These ignore `<base>` and break under the preview prefix and GitHub Pages project paths.
- DeployPage has no Preview button.
- `build_writer` copies configured media into `out/media`, but the console has no upload endpoint and no route serving live `data/media`.
- A stored `/media/...` URL currently falls through the Admin static handler and returns `admin.html`, not an image.

## 3. Built preview semantics

Built preview displays the exact last successful `out_dir` state. It does not include unsaved or unpublished drafts unless they were included by the static build contract.

The UI labels it **Preview Site** and distinguishes it from per-item **Draft Preview**.

## 4. Public static HTML base contract

### 4.1 Admin and public assets differ

- Live Admin HTML remains root-hosted with `/assets/...` path-absolute tags.
- Public static HTML must contain relative `assets/...` tags.
- A global Vite `base: "./"` is not used because the dual-entry live build includes Admin and would change Admin behavior.

### 4.2 Build writer materialization

`build_writer` extracts tags from the static public embed and materializes public HTML:

1. Convert only public script/style asset URLs from `/assets/...` to `assets/...`.
2. Insert `<base href="{deployment_base}">` before dependent script/link tags.
3. Apply the same transformation to root, collection, detail, search, and `404.html` shells.
4. Keep canonical links absolute from `site.base_url`.

Because public asset tags are relative, `<base>` controls both project Pages and preview prefixes. A leading slash is forbidden in materialized public asset/data/media URLs.

### 4.3 Runtime URL resolver

Public JS uses:

```ts
resolvePublicAsset(relativePath: string): URL {
  return new URL(relativePath.replace(/^\/+/, ""), document.baseURI);
}
```

It resolves `data/blog.json`, `media/profile/a.webp`, and internal static resources. The public router derives its basename from the pathname of `document.baseURI`.

## 5. Build manifest

Each build writes:

```text
out/.oxibuilder-build.json
```

```json
{
  "build_id": "uuid",
  "deployment_base": "/repo/",
  "theme_id": "paper",
  "asset_revision": "sha256",
  "built_at": "RFC3339"
}
```

`BuildManifest` is a serialized Rust type in `oxibuilder-core::build_manifest` with `read_from(out_dir)` and atomic `write_to(out_dir)` helpers. Build writer, preview, console preflight, CLI deploy, and `oxibuilder-deploy` all consume this one type; no subsystem reparses ad hoc JSON.

Preview requires this file. A missing manifest or missing `out/index.html` returns `424 build_required` rather than serving a partial directory.

## 6. Preview handler

Canonical prefix:

```text
/api/console/preview/{percent-encoded-slug}/
```

Resolution:

```text
empty path                         → out/index.html
directory path                    → <dir>/index.html
existing file                     → exact file
missing client-side route         → out/404.html
missing build/manifest/index      → 424 build_required
```

For HTML responses only, replace the generated `<base href="{deployment_base}">` with:

```text
/api/console/preview/{encoded-slug}/
```

Do not rewrite JS, CSS, JSON, images, or canonical links.

Security and response rules:

- reject `.` and `..` path components before filesystem access,
- reject percent-decoded traversal, encoded separators, NUL, and invalid UTF-8,
- canonicalize the candidate and require it to remain under `out_dir`,
- MIME from final path,
- `X-Content-Type-Options: nosniff`,
- `Cache-Control: no-store`,
- inline rendering for supported web content; no directory listing.

A dedicated root route handles the no-wildcard path so `/preview/{slug}` redirects to `/preview/{slug}/`.

## 7. DeployPage preview UX

Header actions:

```text
[Build] [Preview Site ↗] [Deploy]
```

- Preview is disabled without a compatible build manifest.
- Clicking opens a new tab at the preview root.
- A collapsible in-page panel may embed the same URL in a sandboxed iframe:
  ```text
  sandbox="allow-scripts allow-same-origin allow-popups"
  ```
- Header shows build ID, build time, theme, and deployment base.
- Successful build invalidates build status and preview readiness.
- A stale build badge appears when current theme/deploy base differs from the manifest.

## 8. Media storage and logical path

Uploaded media lives under:

```text
SiteContext.media_dir/<extension>/<uuid>.<ext>
```

Content rows store either:

```text
https://external.example/image.webp
media/<extension>/<uuid>.webp
```

Site media paths never begin with `/`. This prevents root-origin coupling and allows a context resolver to choose the correct URL.

## 9. Media API

### Upload

```text
POST /api/console/s/{slug}/media/{extension}
Content-Type: multipart/form-data
```

Success:

```json
{
  "data": {
    "path": "media/profile/02c4….webp",
    "mime": "image/webp",
    "bytes": 42133
  }
}
```

Rules:

- allow JPEG, PNG, WebP, GIF,
- exclude SVG,
- limit each request/file to 10 MiB,
- validate magic bytes independently of declared MIME and filename,
- choose extension from detected MIME,
- validate extension ID against the registered extension registry and safe path syntax,
- use UUID filenames,
- stream to a temporary file within the final filesystem and atomically rename,
- never use a user filename as a path component,
- delete a partial temp file on every failure.

`axum` enables its `multipart` feature for `Multipart`; the console adds a streaming body utility and an image-signature crate or a small explicit signature validator. The implementation must not buffer the complete 10 MiB body merely to identify the format.

### Live serving

```text
GET /api/console/s/{slug}/media/{extension}/{file}
HEAD /api/console/s/{slug}/media/{extension}/{file}
```

- Serve from `SiteContext.media_dir` before the Admin SPA fallback.
- Apply the same canonical containment checks as preview.
- Set accurate MIME, `nosniff`, and `Cache-Control: no-cache`.
- Unknown file returns 404; it never returns `admin.html`.

No delete endpoint is added in this subproject. Upload cancellation can leave an unreferenced file; automatic deletion risks removing a file referenced by another content row. A future media library may manage orphans.

## 10. Asset resolver contracts

```ts
interface AssetResolver {
  resolve(value: string | null): string | null;
}
```

Resolvers:

- `adminAssetResolver(slug)`: `media/...` → `/api/console/s/{slug}/media/...`
- `previewAssetResolver(previewBase)`: `media/...` → `new URL(mediaPath, previewBase)`
- `publicAssetResolver`: `media/...` → `new URL(mediaPath, document.baseURI)`
- absolute `http:`/`https:` values pass through unchanged.

Reject unsupported schemes such as `javascript:`, `data:`, and `file:`.

## 11. Shared ImageField

`ImageField` provides:

- URL/media-path input,
- Upload button and hidden file input,
- progress/pending state,
- thumbnail through the Admin resolver,
- MIME/size metadata,
- Clear action,
- field-level error.

It does not own form persistence. Its `onChange` returns the stored logical path or external URL.

Initial adopters:

- Profile avatar,
- Novels cover,
- Books cover,
- Links thumbnail,
- Projects screenshots,
- Movies series cover.

TMDB/Aladin external image URLs remain valid.

## 12. Build integration

The existing media copy remains conceptually:

```text
SiteContext.media_dir → SiteContext.out_dir/media
```

It runs after a clean output directory is created. The generated public resolver loads the copied files relative to the build base.

## 13. Dependencies

```text
workspace axum features             # add multipart
oxibuilder-console                     # body streaming/signature validation dependency
```

Use existing `uuid`, `tokio`, and `mime_guess`. Do not add an image transformation library because resize/conversion is out of scope.

## 14. File map

```text
crates/oxibuilder-console/src/
├── media/
│   ├── mod.rs
│   ├── upload.rs
│   └── serve.rs
├── preview/handler.rs
└── per_site.rs                    # media routes/status

crates/oxibuilder-core/src/
├── build_manifest.rs              # shared typed manifest/read/write
├── build_writer.rs                # relative public tags, <base>, manifest, media copy
└── build.rs                       # build metadata input

web/src/
├── shared/api.ts                  # document.baseURI static resolver
├── shared/assets.ts               # AssetResolver
└── admin/
    ├── shared/api.ts              # upload/status clients
    ├── shared/ui/ImageField.tsx
    └── deploy/DeployPage.tsx      # Preview Site
```

## 15. Verification

- Public materialized HTML contains `assets/...`, never `/assets/...`, and places `<base>` before scripts/styles.
- Root and project deployment bases load JS, CSS, data, routes, and media.
- Preview root, collection, detail, asset, data, media, and unknown route resolve under the preview prefix.
- Preview traversal attempts, including percent-encoded forms, are rejected.
- Preview has no-store and never serves outside `out_dir`.
- PNG/JPEG/WebP/GIF uploads round-trip; SVG, executable content, MIME spoofing, and files over 10 MiB are rejected.
- Live uploaded media returns image bytes rather than the Admin SPA.
- One uploaded image appears immediately in Admin, then in built preview after build.
