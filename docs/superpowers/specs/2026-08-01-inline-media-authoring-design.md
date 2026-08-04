# Inline Media Authoring — Design Spec

> **Date:** 2026-08-01
> **Predecessor:** Console Preview + Media (subproject 3, completed)
> **Scope:** Insert images and GIFs *into* markdown content bodies (blog posts,
> project descriptions, profile bios, book/movie/scraps reviews, novel chapters)
> and render them correctly in every context.
> **Decision mode:** Autonomous — user authorized full design-to-implementation
> execution and is unavailable for review gates. Decisions are recorded inline
> with rationale.

## 1. Goal

Let the owner embed images and animated GIFs **inline within markdown bodies** —
not just as single cover/avatar fields — and have them render correctly across
all four rendering contexts: live Admin, draft preview (Admin editor), built
preview (iframe), and the deployed public site.

This is an **extension of the existing per-site media module**, not a new
storage layer. Storage, upload, serve, build-copy, and four-context asset
resolvers already ship (subproject 3).

## 2. Verified current state

The media foundation is complete and committed:

- **Storage:** `<data_dir>/media/<extension>/<uuid>.<ext>` per site.
- **Upload** (`media/upload.rs`): multipart, magic-byte MIME detection
  (JPEG/PNG/WebP/**GIF** — GIF already works), 10 MiB streaming cap, atomic
  rename, UUID filenames, safe-extension-id validation.
- **Serve** (`media/serve.rs`): `GET/HEAD /media/{ext}/{file}`, path-traversal
  containment, accurate MIME, `nosniff`, `no-cache`.
- **Build copy** (`build_writer.rs`): `media_dir → out/media/`.
- **Resolvers** (`web/src/shared/assets.ts`): `adminAssetResolver(slug)`,
  `previewAssetResolver(base)`, `publicAssetResolver()` — handle `media/...`
  logical paths + external URLs, reject unsafe schemes.
- **AssetResolverContext** (`EditorPreviewDrawer.tsx`): provided around `*View`
  components in the preview drawer; `useAssetResolver()` hook exists but is
  **in-file / admin-only**.
- **ImageField** (`admin/shared/ui/ImageField.tsx`): single-value upload for
  cover/avatar/thumbnail fields.

**Markdown rendering** is client-side, split across two libraries:

- **Public SPA** (`shared/Markdown.tsx`): `markdown-it`. Used by `BlogPostView`,
  `ProfileView`, `ProjectView`.
- **Admin editor preview** (`admin/shared/ui/MarkdownEditor.tsx`): `marked`.

`MarkdownEditor` is the universal body editor, used in **6 tabs**: Blog (body),
Books (review ko/en), Movies (review ko/en), Novels (chapter body), Profile
(bio ko/en), Scraps (note ko/en).

## 3. The three gaps

### G1 — Markdown rendering ignores asset resolvers

`Markdown.tsx` renders `![alt](media/blog/uuid.webp)` → `<img src="media/blog/uuid.webp">`.
On a nested detail route (`/blog/<slug>/`) this relative URL mis-resolves. The
public resolvers exist precisely to fix this (they use `document.baseURI`, which
`<base>` pins to the deployment base), but **Markdown never calls them**. Same
for the `marked` preview in `MarkdownEditor`. Cover images work because
`ImageField`/`*View` use the resolver; inline body images do not.

### G2 — No editor insertion affordance

`MarkdownEditor` is a plain `<Textarea>` + preview toggle. There is no way to
upload an image and splice `![alt](media/<ext>/<uuid>.<ext>)` at the cursor, no
drag-and-drop, no paste, no media picker. The only upload path is `ImageField`,
which binds one image to one field — it cannot insert into running prose.

### G3 — No media library (list / delete / pick)

Only upload + serve exist. There is no endpoint to enumerate uploaded media, no
way to browse/reuse an existing asset, and no delete. Orphans accumulate
(subproject 3 explicitly deferred delete). A content author cannot pick a
previously-uploaded image without re-uploading.

### G4 — Build-path correctness (verification, not new work)

Once G1 routes image srcs through `publicAssetResolver()` (which resolves against
`document.baseURI`), inline media resolves on nested built/deployed pages because
`build_writer` inserts `<base href="{deployment_base}">`. This holds for root
and project-pages deployment bases. **A regression test is added** to lock it in.

## 4. Approaches considered

### A. Resolver-aware Markdown + MediaPicker + editor insertion (CHOSEN)

Promote `AssetResolverContext` to shared; make both markdown renderers resolve
`media/...` image srcs via the active resolver; add a list+delete backend; build
a reusable `MediaPicker`; extend `MarkdownEditor` with upload/drag-drop/paste +
picker toolbar.

- *Pro:* minimal new surface; reuses the exact resolver model covers already use;
  correct in all 4 contexts by construction; every markdown body benefits.
- *Con:* touches the shared markdown component and 6 editor callsites (to pass
  `slug`/`extension`).

### B. New "asset repository" table + CMS media manager

Introduce a `media_assets` table with metadata, tags, reference counts; a full
DAM.

- *Pro:* reference-aware safe delete; rich metadata.
- *Con:* reinvents storage the filesystem already provides; large scope; the
  user's actual need is *inserting images into bodies*, not a DAM. **Rejected as
  over-scope** (YAGNI). The filesystem + list endpoint gives 90% of the value.

### C. External-only images (paste any URL, no upload)

Skip local media; only support `![alt](https://...)`.

- *Pro:* zero backend work.
- *Con:* the user explicitly wants an on-site asset store ("고유의 에셋 저장소");
  external-only loses self-hosting, breaks offline, and leaves GIFs/hosting to a
  third party. **Rejected.**

**Decision: Approach A.** It directly serves "insert images/GIFs into bodies
with a per-site asset store" while reusing the substantial existing module.

## 5. Design

### 5.1 Shared asset-resolver context

Promote the resolver plumbing out of the admin-only `EditorPreviewDrawer.tsx`
into **`web/src/shared/asset-context.tsx`**:

- `AssetResolverContext`, `AssetResolverProvider`, `useAssetResolver()`.
- `useAssetResolver()` default: `publicAssetResolver()` — the safe default for
  the public SPA when no provider is mounted.

Wiring:

- **Public SPA root** (`App.tsx`): wrap the route tree in
  `<AssetResolverProvider resolver={publicAssetResolver()}>`.
- **Admin editor preview drawer** (`EditorPreviewDrawer.tsx`): keep providing the
  admin/preview resolver (re-export from shared; delete the in-file copy).

### 5.2 Resolver-aware Markdown rendering

**`shared/Markdown.tsx`** (markdown-it): build the instance per-render so the
resolver closure is fresh, and install an `image` renderer rule that rewrites
**only** `media/...` srcs:

```ts
const isMediaRef = (src: string) =>
  /^\/?media\//.test(src.trim());

// renderer rule:
const src = token.attrGet("src") ?? "";
if (isMediaRef(src)) {
  const r = resolver.resolve(src);
  if (r) token.attrSet("src", r);
}
```

External `http(s)` URLs, anchors, and non-image links are untouched. The
component reads the resolver from `useAssetResolver()`.

**`admin/shared/ui/MarkdownEditor.tsx`** (marked): add an `extension` + `slug`
prop; build `marked` with a `renderer.image` override that resolves `media/...`
via `adminAssetResolver(slug)`. The preview pane then shows inline images live.

### 5.3 Media library backend (list + delete)

Extend `media/mod.rs` router and add `media/library.rs`:

```
GET    /api/console/s/{slug}/media              → list
GET    /api/console/s/{slug}/media?extension=blog → filtered list
DELETE /api/console/s/{slug}/media/{extension}/{file} → remove
```

**List response:**

```json
{
  "data": [
    { "path": "media/blog/02c4….webp", "extension": "blog",
      "file": "02c4….webp", "mime": "image/webp",
      "bytes": 42133, "updated_at": "2026-08-01T00:00:00Z" }
  ]
}
```

Implementation: walk `media_dir` one level (`<extension>/<file>`), reuse the
existing component-containment checks, derive MIME via `mime_guess`, sort by
mtime descending. `extension` query filters the top-level dir.

**Delete:** validate `{extension}` and `{file}` with the **same** containment
logic as `serve.rs` (component-normal, canonicalize under `media_dir`), then
`tokio::fs::remove_file`. Return `204`. Missing file → `404`.

**Delete safety:** This is a 1-person-owner tool (doc §0.3 security model).
Delete is **manual with a confirmation modal** in the picker. No reference
tracking in this iteration — the modal warns "may break references in content."
Reference-aware delete (scan text columns) is noted as a future enhancement,
out of scope here.

### 5.4 MediaPicker component

New reusable modal **`admin/shared/ui/MediaPicker.tsx`**:

- Thumbnail grid of the site's media (via the list endpoint, resolved through
  `adminAssetResolver(slug)`).
- Filter chips by extension; default to the editor's extension.
- **Upload** button inside the picker (reuse `uploadImage`).
- **Delete** button per item (confirmation modal).
- Click an item → calls `onPick(path)` with the logical `media/...` path.

Props: `{ slug, extension?, onPick(path), trigger }`. Used by `MarkdownEditor`
toolbar and (optionally) upgrade `ImageField` to offer "pick from library."

### 5.5 MarkdownEditor insertion

Extend `MarkdownEditor` props: add `slug`, `extension`. Add a toolbar row above
the textarea:

```
[Image ▾]   Edit | Preview
```

Insertion paths, all producing `![<name>](media/<ext>/<uuid>.<ext>)` spliced at
the textarea selection:

1. **Toolbar → MediaPicker:** open picker, on pick splice at cursor.
2. **Drag-and-drop** image file onto the textarea → upload → splice.
3. **Paste** image file (`clipboardData.files`) → upload → splice.
4. **Paste/enter external URL:** if the user types an `http(s)` image URL it is
   left as-is (no upload) — markdown-it/marked render it directly.

Splicing preserves the textarea selection via a ref + `setSelectionRange` after
state update. `alt` defaults to the uploaded filename stem.

### 5.6 Callsite updates

Pass `slug` + `extension` to `MarkdownEditor` in all 6 tabs:

| Tab | Field | `extension` |
|---|---|---|
| Blog | body | `blog` |
| Novels | chapter body | `novels` |
| Profile | bio ko/en | `profile` |
| Books | review ko/en | `books` |
| Movies | review ko/en | `movies` |
| Scraps | note ko/en | `scraps` |

**Projects:** `ProjectView` renders `description_ko/en` via `Markdown.tsx`, so
inline images render once G1 is fixed. Audit `ProjectsTab.tsx`: if its
description editor is a plain `Textarea`, upgrade it to `MarkdownEditor` with
`extension="projects"` for insertion parity. (Verified during planning.)

`ImageField` upgrade (pick from library) is **optional polish**; not required for
the core goal.

### 5.7 Build integration

No change. The existing `media_dir → out/media/` copy already ships every
uploaded asset, including inline ones. G1's resolver fix is what makes them
resolve under the deployment base.

## 6. Data flow (one image, end to end)

```
Admin editor
  drag GIF onto textarea
   → uploadImage(slug,"blog",file)
   → POST /api/console/s/{slug}/media/blog   (magic-byte GIF, 10MiB)
   → stores media/blog/<uuid>.gif, returns {path:"media/blog/<uuid>.gif"}
   → splice ![name](media/blog/<uuid>.gif) at cursor
   → save post (body stored verbatim in SQLite)

Admin preview (marked)
  renderer.image resolves media/blog/<uuid>.gif
   → /api/console/s/{slug}/media/blog/<uuid>.gif  ✓ visible

Public built site
  oxibuilder build → copies media/blog/<uuid>.gif → out/media/blog/<uuid>.gif
  SPA renders markdown → Markdown.tsx image rule
   → publicAssetResolver().resolve("media/blog/<uuid>.gif")
   → new URL("media/blog/<uuid>.gif", document.baseURI)
   → <deployment_base>/media/blog/<uuid>.gif   ✓ correct on /blog/<slug>/
```

## 7. Error handling

- **Upload failures** (oversize, bad magic bytes, spoofed MIME): already
  rejected by `upload.rs`; the editor surfaces the message inline and aborts the
  splice (no partial markdown inserted).
- **Drag/paste of non-images** (e.g. PDF): client rejects (type check) before
  upload; no splice.
- **Delete of in-use asset:** allowed (manual); the modal warns. The referenced
  `<img>` will 404 — the editor preview makes breakage immediately visible.
- **Unresolved media ref** (resolver returns null for an unsafe scheme):
  markdown-it leaves the original src; `nosniff` + browser blocking handles it.

## 8. Testing

**Rust (console):**
- `GET /media` lists uploaded files with mime/bytes; `?extension=` filters.
- `DELETE /media/{ext}/{file}` removes the file; traversal (`..`, encoded) → 400;
  missing → 404; canonicalization must stay under `media_dir`.
- Existing upload/serve tests remain green.

**TypeScript / build:**
- `npx tsc --noEmit` clean.
- `bun run build` succeeds (embedded SPA rebuilt; `build.rs` re-embeds).

**Build-path correctness (regression):**
- After `oxibuilder build`, a post body containing `![](media/blog/x.webp)` renders
  an `<img>` whose resolved URL equals `<deployment_base>/media/blog/x.webp`,
  not a nested-page-relative path. Asserted via the resolver unit behavior +
  an SSG test that the public HTML carries `<base>` before assets.

**Browser smoke (manual, final):**
- Open console, create/edit a blog post, drag a GIF into the body, confirm it
  appears in the editor preview, save, open the public draft preview, confirm it
  renders; run `oxibuilder build` and confirm the built preview renders the GIF on
  the detail route.

## 9. File map

```
crates/oxibuilder-console/src/media/
├── mod.rs              # add GET /media, DELETE /media/{ext}/{file}
└── library.rs          # list_handler, delete_handler (NEW)

crates/oxibuilder-console/tests/
└── media.rs            # add list + delete tests

web/src/
├── shared/
│   ├── asset-context.tsx   # promoted context + provider + hook (NEW)
│   └── Markdown.tsx        # resolver-aware image rule
├── App.tsx                 # wrap public tree in AssetResolverProvider
└── admin/
    ├── shared/
    │   ├── api.ts               # listMedia, deleteMedia clients
    │   └── ui/
    │       ├── MarkdownEditor.tsx  # toolbar + drag/paste/picker + resolver
    │       ├── MediaPicker.tsx     # library modal (NEW)
    │       └── EditorPreviewDrawer.tsx  # re-export context from shared
    └── content/
        ├── BlogTab.tsx, NovelsTab.tsx, ProfileTab.tsx,
        ├── BooksTab.tsx, MoviesTab.tsx, ScrapsTab.tsx   # pass slug+extension
        └── ProjectsTab.tsx   # upgrade description editor if plain textarea
```

## 10. Out of scope

- Reference-aware safe delete (scan content text columns before removal).
- Image resizing / format conversion / thumbnail generation.
- Per-asset metadata, tags, folders, or a full DAM.
- SVG support (intentionally excluded by the existing upload validator).
- Bulk upload / zip import.

## 11. Acceptance criteria

1. In any `MarkdownEditor`, an image/GIF can be uploaded via drag-drop, paste, or
   the picker, and the markdown `![alt](media/<ext>/<uuid>.<ext>)` is spliced at
   the cursor.
2. The editor preview renders the inline image via the admin resolver.
3. The media picker lists all site media, filters by extension, uploads, and
   deletes (with confirmation).
4. A saved blog/project/profile body with inline media renders the image on the
   public draft preview **and** the built/deployed site, including on nested
   detail routes (`/blog/<slug>/`).
5. `cargo test`, `tsc --noEmit`, and `bun run build` all pass.
