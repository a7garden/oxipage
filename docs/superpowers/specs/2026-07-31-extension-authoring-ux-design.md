# Extension Authoring UX — Design Spec

> **Date:** 2026-07-31
> **Subproject:** 5 of 5
> **Predecessors:** Admin theme system; built preview/media; GitHub Pages base contract
> **Scope:** built-in extension forms, unsaved preview, Profile admin, validation and bounded UX fixes

## 1. Goal

Let authors see how unsaved content will look before publishing, upload images instead of requiring external URLs, manage the Profile extension after setup, and receive precise field-level validation across every built-in extension form.

## 2. Architecture decision

Do not introduce a generic `Extension::admin_forms()` manifest in this suite. Built-in editors contain structurally different workflows—chapters, series, screenshots, external search, Profile repeaters—and forcing them into a declarative CRUD schema would increase complexity and weaken UX.

Keep explicit editors and extract only behavior with a stable shared contract:

```text
EditorPreviewDrawer
DraftPreviewPane
ImageField
TagInput
FieldError
AssetResolverContext
validation utilities
```

## 3. Actual renderer reuse

Public pages currently mix data fetching and presentation. Split each adopted surface:

```tsx
function BlogPostPage() {
  const post = useBlogPost();
  return <BlogPostView post={post} />;
}

function BlogPostView({ post, assets, language }: Props) {
  // presentation only; no fetch
}
```

Presentation components:

- receive complete typed data,
- do not issue API calls,
- resolve media through `AssetResolverContext`,
- accept language/theme context,
- are used by both public pages and Admin DraftPreviewPane.

Admin preview feeds a view model derived directly from local form state. It does not save a hidden draft or call a draft-render endpoint.

## 4. Editor/preview layout

`EditorPreviewDrawer` replaces the narrow form-only drawer where faithful preview is required.

Desktop:

```text
┌──────────── Editor ────────────┬──────────── Preview ───────────┐
│ fields                         │ actual public view             │
│ upload                         │ KO / EN switch                 │
│ validation                     │ current site theme             │
└────────────────────────────────┴────────────────────────────────┘
```

Smaller viewport:

```text
[Edit] [Preview]
```

Behavior:

- preview updates on every local field change,
- no save/publish is required,
- current site's public theme is scoped to the preview pane only,
- uploaded site media resolves through the site-scoped Admin media endpoint,
- external URLs pass through,
- empty/invalid fields render a controlled placeholder instead of crashing,
- MarkdownEditor remains useful for Markdown syntax preview; DraftPreviewPane shows final component layout.

`Preview Site` remains the last static build; `Draft Preview` remains one unsaved item. UI copy always distinguishes them.

## 5. Profile Admin

Add Profile as the eighth Content tab. It uses existing site-scoped `GET/PUT /profile` and manages the full singleton:

- display name,
- KO/EN tagline,
- avatar via `ImageField`,
- KO/EN bio via `MarkdownEditor`,
- email,
- GitHub username,
- LinkedIn URL,
- repeatable education:
  - institution, degree, field, start year, end year,
- repeatable custom links:
  - label, URL, icon.

The public `ProfilePage` is split into fetch wrapper plus `ProfileView`; Admin reuses `ProfileView`.

Setup wizard remains a short initial configuration flow and writes the same row. It is not reused as the long-term editor. The Admin form initializes from a complete GET before enabling Save so full-replace PUT cannot erase unseen fields.

### Concurrency safety

Profile GET returns `updated_at`. PUT includes:

```json
{"expected_updated_at":"...", "profile":{...}}
```

If the row changed since load, return 409 `stale_profile`; Admin keeps local values and offers Reload/Compare rather than overwriting silently.

## 6. Extension-specific improvements

### 6.1 Blog

- Use `BlogPostView` for Draft Preview.
- Replace comma-separated tags with `TagInput` while preserving `string[]` API payload.
- Language options come from site config languages rather than hard-coded `ko/en`.
- Required: trimmed title; selected language must remain enabled for the site.
- Slug remains server-generated; manual slug editing is out of scope.

### 6.2 Projects

- Use shared project card/detail presentation.
- Add links repeatable editor for the existing links JSON field.
- Expose started/ended dates already present in the model.
- Replace screenshot URL-only input with `ImageField` while still accepting external URLs.
- Add both KO and EN alt text.
- Validate at least one localized title, supported status, and `ended_at >= started_at`.
- Replace paired screenshot reorder PATCHes with atomic order API.

### 6.3 Links

- Use the real public link card.
- Thumbnail uses `ImageField`.
- Tags use `TagInput`.
- Validate required http(s) destination, optional thumbnail as http(s) or site media path, and integer display order.
- No publish action is added because Link is currently immediate content, not draft/publish content.

### 6.4 Movies and series

- Use the public movie card for Draft Preview.
- Connect the existing TMDB search endpoint to a `Search → Select → Edit → Save draft` flow.
- Selection prefills title, media type, release year, poster path; author can edit supported local fields.
- Series editor exposes title, cover `ImageField`, rating, and reviews already supported by the model.
- Validate media type, title/TMDB requirement, integer rating 0–10, plausible release year, positive series order.
- Remove `any` casts by aligning Admin types with API response types.

### 6.5 Books

- Use the public book card.
- Connect existing Aladin search to `Search → Select → Edit`.
- Cover uses `ImageField` and external provider URL remains accepted.
- Reviews use MarkdownEditor.
- Correct UI enum mismatch:
  ```text
  wishlist | reading | completed | dropped
  ```
  Remove UI-only `read` and `dnf` values.
- Validate rating 0–10, ISBN-13 checksum/format when present, and `finished_at >= started_at`.

### 6.6 Novels and chapters

- Use the public novel card; chapter preview uses the actual chapter prose presentation.
- Cover uses `ImageField`; tags use `TagInput`.
- Chapter title is required before add/save.
- Replace the two independent reorder PATCH calls with one atomic order API.

### 6.7 Scraps

- Use the real public scrap card.
- Keep scraped OG image as derived metadata but add an explicit image override using `ImageField`.
- Use TagInput.
- Distinguish read-only source/scraped fields from editable notes/tags/image override.
- Validate required http(s) source URL and server source enum.
- Re-scrape is out of scope.

## 7. Atomic reorder APIs

### Novel chapters

```text
PUT /api/console/s/{site}/novels/{novel}/chapters/order
{"chapter_ids":[12,7,30]}
```

### Project screenshots

```text
PUT /api/console/s/{site}/projects/{project}/screenshots/order
{"screenshot_ids":[4,9,2]}
```

Server requirements:

- submitted IDs equal the complete current child set exactly,
- no duplicates or unknown IDs,
- update all display orders in one DB transaction,
- return the ordered list,
- stale/incomplete list returns 409.

This removes the current transient duplicate-order/race behavior.

## 8. Validation contract

Server validation is authoritative. Client validation provides immediate matching feedback.

Error shape:

```json
{
  "error": {
    "code": "validation_error",
    "field": "cover_image",
    "message": "Must be an http(s) URL or site media path"
  }
}
```

`jsonOrThrow` preserves it as:

```ts
class ApiValidationError extends Error {
  code: string;
  field: string;
}
```

`DrawerField` gains an `error` prop. Errors render under the responsible field, not as one generic footer string.

Shared rules:

```text
external URL = http:// or https://
media path   = media/<registered-extension>/<safe-file>; no leading slash, . or ..
image value  = external URL or media path
rating       = integer 0..10
year         = bounded four-digit integer
date range   = end >= start
localized required pair = at least one non-empty value
email        = syntactically valid address when present
```

Add missing server checks for image fields, Profile email/URLs/year order, ISBN-13, and date ranges. Client helpers match the accepted syntax but never replace server validation.

## 9. Editing-state safety

- Track whether the form differs from loaded/empty initial state.
- Closing the drawer or navigating away with dirty state asks to discard.
- Disable close and duplicate submit while the save mutation is committing; upload can fail independently.
- Save success updates query cache and then closes.
- Save failure preserves form and preview.
- Publish reuses server validation and reports field failures when applicable.
- Image upload success changes only the image field; it does not save the whole content row.

## 10. API/client additions

```text
POST/GET media routes              # from preview/media subproject
PUT profile with expected_updated_at
PUT chapters/order
PUT screenshots/order
```

Client additions:

```ts
uploadImage(slug, extension, file)
getProfile(slug)
updateProfile(slug, profile, expectedUpdatedAt)
reorderChapters(slug, novelSlug, ids)
reorderScreenshots(slug, projectSlug, ids)
searchTmdb(slug, query)
searchAladin(slug, query)
```

Draft preview of unsaved state needs no backend draft endpoint.

## 11. File map

```text
web/src/
├── shared/
│   ├── assets.ts
│   └── extension presentation components # fetch/view split
└── admin/
    ├── content/
    │   ├── ContentPage.tsx                 # Profile tab
    │   ├── ProfileTab.tsx
    │   └── *Tab.tsx                        # explicit editors
    ├── shared/
    │   ├── api.ts
    │   ├── validation.ts
    │   └── ui/
    │       ├── EditorPreviewDrawer.tsx
    │       ├── DraftPreviewPane.tsx
    │       ├── ImageField.tsx
    │       └── TagInput.tsx
    └── shared content primitives

crates/oxibuilder-ext-profile/src/routes.rs
crates/oxibuilder-ext-projects/src/{routes.rs,repo.rs}
crates/oxibuilder-ext-novels/src/{routes.rs,repo.rs}
crates/oxibuilder-ext-*/src/routes.rs              # missing validation
```

## 12. Explicit non-goals

- Generic extension form manifest/renderer.
- Media library, deletion, deduplication, or orphan GC.
- Image resizing, thumbnail generation, or format conversion.
- Autosave, revision history, or collaborative editing.
- Manual blog slug editing.
- New external providers beyond existing TMDB/Aladin endpoints.
- Re-scraping scraps.

## 13. Verification

- Every form's Draft Preview updates before Save and uses the same presentation component as public pages.
- Profile all fields round-trip; public page reflects them; setup and Admin write the same singleton without field loss.
- Stale Profile PUT returns 409 and preserves local changes.
- Media images display in Admin preview and after static build.
- Valid boundaries and invalid cases for URL/media path, rating, date, year, ISBN, email, bilingual title, and status enums behave consistently client/server.
- Books never send the obsolete `read`/`dnf` states.
- TMDB and Aladin selection prefills a draft without immediately publishing.
- Chapter and screenshot reorder are atomic, reject duplicate/incomplete ID lists, and never leave duplicate order values.
- Dirty drawers warn on close; failed mutation/upload preserves form state.
