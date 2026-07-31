# Extension Authoring UX — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let authors preview unsaved extension content, manage the Profile singleton from Admin with optimistic-concurrency safety, upload site media into every image field, and receive precise field-level validation across every built-in extension form.

**Architecture:** Built-in extension editors stay explicit (chapters, series, screenshots, external search, Profile repeaters); no generic manifest. We extract reusable contracts — `EditorPreviewDrawer`, `DraftPreviewPane`, `ImageField`, `TagInput`, validation helpers, `ApiValidationError`, AssetResolver — and split each public page into fetch + `*View` so the Admin preview reuses the same presentation component as the public site. Server validation is authoritative; client validation mirrors it for instant feedback.

**Tech Stack:** React 19, TypeScript, Vite 7, TanStack Query 5, axum 0.8, sqlx, serde

## Global Constraints

- Built-in editors remain explicit; no `Extension::admin_forms()` manifest in this suite.
- Presentation components (`*View`, `*Card`) receive complete typed data, do not call APIs, resolve media through `AssetResolverContext`, and accept a `language` prop.
- Admin preview feeds a view model derived from local form state — never a draft-render endpoint.
- Shared contracts depend on artifacts from peer plans. Where this plan uses one, it consumes the interface and does NOT redefine it:
  - `ImageField`, `uploadImage(slug, extension, file)`, `AssetResolver`, `AssetResolverContext` — produced by `docs/superpowers/plans/2026-07-31-console-preview-media-plan.md`.
  - `adminAssetResolver(slug)`, `previewAssetResolver(previewBase)`, `publicAssetResolver` — produced by the preview/media plan.
  - Admin theme system (`PublicThemeScope`) — produced by `docs/superpowers/plans/2026-07-31-admin-theme-system-plan.md`.
- Post-foundation site context (per parent correction): `SiteContext` exposes `settings: Arc<RwLock<MutableSiteSettings>>` and `startup_server: ServerConfig`. `config` is removed. Per-site extension handlers reach the mutable site fields via `ctx.settings.read().await.site.*` and startup-immutable server fields via `ctx.startup_server.*`. `MutableSiteSettings` shape (unchanged):
  ```rust
  pub struct MutableSiteSettings {
      pub site: MutableSiteConfig { name, base_url, default_lang, languages },
      pub lobby: MutableLobbyConfig { default_mode },
      pub integrations: MutableIntegrationsConfig { github_username, tmdb_api_key_env, aladin_ttbkey_env },
      pub extensions: MutableExtensionsConfig { enabled },
      pub deploy: DeployConfig { github_pages: Option<GitHubPagesTarget> },
  }
  ```
  `GitHubPagesTarget` keeps `owner`, `repo`, `branch` as `String` fields.
- API contract: every `ApiError::validation(field, message)` response must surface as a typed `ApiValidationError` carrying `{ code, field, message }`. Client helpers mirror server rules but never replace them.
- Error contract from spec §8:
  ```ts
  class ApiValidationError extends Error { code: string; field: string }
  ```
- Shared validation rules (single source of truth — spec §8):
  ```text
  external URL = http:// or https://
  media path   = media/<registered-extension>/<safe-file>; no leading slash, . or ..
  image value  = external URL or media path
  rating       = integer 0..10
  year         = bounded four-digit integer
  date range   = end >= start
  email        = syntactically valid address when present
  ```
- Books `status` enum: `wishlist | reading | completed | dropped` (no `read`/`dnf`).
- Projects `status` enum: `active | archived | wip` (already correct).
- Movies `media_type` enum: `movie | tv`.
- Scraps `source` enum: `hackernews | geeknews | manual`.
- Profile PUT carries `expected_updated_at`; stale writes return 409 `stale_profile`.
- Atomic reorder endpoints require submitted IDs to equal the complete current child set exactly (no duplicates, no unknown); incomplete lists return 409 `stale_order`.
- Dirty drawers warn on close; failed mutation/upload preserves form + preview.
- No placeholders, no shims, no aliases after cutover.

---

## File Structure

```text
web/src/
├── extensions/
│   ├── blog/
│   │   ├── BlogPostPage.tsx             # MOD: render BlogPostView
│   │   ├── BlogPostView.tsx             # NEW: presentation only
│   │   ├── BlogPostCard.tsx             # NEW
│   │   └── BlogListPage.tsx             # MOD: render BlogPostCard
│   ├── books/
│   │   ├── BooksPage.tsx                # MOD: render BookCard
│   │   └── BookCard.tsx                 # NEW
│   ├── links/
│   │   ├── LinksPage.tsx                # MOD: render LinkCard
│   │   └── LinkCard.tsx                 # NEW
│   ├── movies/
│   │   ├── MoviesPage.tsx               # MOD: render MovieCard
│   │   └── MovieCard.tsx                # NEW
│   ├── novels/
│   │   ├── NovelsPage.tsx               # MOD: render NovelCard
│   │   └── NovelCard.tsx                # NEW
│   ├── profile/
│   │   ├── ProfilePage.tsx              # MOD: render ProfileView
│   │   └── ProfileView.tsx              # NEW: presentation only
│   ├── projects/
│   │   ├── ProjectsListPage.tsx         # MOD: render ProjectCard
│   │   ├── ProjectDetailPage.tsx        # MOD: render ProjectView
│   │   ├── ProjectCard.tsx              # NEW
│   │   └── ProjectView.tsx              # NEW: presentation only
│   └── scraps/
│       ├── ScrapsPage.tsx               # MOD: render ScrapCard
│       └── ScrapCard.tsx                # NEW
└── admin/
    ├── shared/
    │   ├── api.ts                       # MOD: ApiValidationError + jsonOrThrow preserves field
    │   ├── validation.ts                # NEW: shared client validators
    │   └── ui/
    │       ├── EditorPreviewDrawer.tsx  # NEW: 2-pane + mobile tabs
    │       ├── DraftPreviewPane.tsx     # NEW: feeds *View from form state
    │       ├── ImageField.tsx           # MOD: error prop wired through (peer plan)
    │       ├── MarkdownEditor.tsx       # unchanged
    │       └── TagInput.tsx             # NEW: chip editor → string[]
    ├── content/
    │   ├── ContentPage.tsx              # MOD: Profile tab in tabs list
    │   ├── ProfileTab.tsx               # NEW
    │   ├── BlogTab.tsx                  # MOD: TagInput, lang from site config, BlogPostView draft preview
    │   ├── BooksTab.tsx                 # MOD: status enum fix, ImageField cover, MarkdownEditor reviews, ISBN-13 validation
    │   ├── LinksTab.tsx                 # MOD: ImageField thumbnail, TagInput
    │   ├── MoviesTab.tsx                # MOD: TMDB search wired, ImageField series cover, remove any
    │   ├── NovelsTab.tsx                # MOD: ImageField cover, TagInput, atomic chapter reorder
    │   ├── ProjectsTab.tsx              # MOD: ImageField screenshots, links repeater, started/ended dates, atomic screenshot reorder
    │   └── ScrapsTab.tsx                # MOD: ImageField og override, TagInput, read-only/editable split

crates/oxipage-core/src/
├── lib.rs                               # MOD: export validation module
└── validation.rs                        # NEW: shared server validators

crates/oxipage-ext-profile/src/
├── model.rs                             # MOD: ProfileInput.expected_updated_at, validate_email/year_order
├── repo.rs                              # MOD: upsert_if_unchanged() with stale detection
└── routes.rs                            # MOD: PUT profile with expected_updated_at → 409 stale_profile

crates/oxipage-ext-novels/src/
├── model.rs                             # MOD: ChapterOrderInput
├── repo.rs                              # MOD: reorder_chapters() in one tx
├── routes.rs                            # MOD: PUT /chapters/order; cover_image validation; 409 stale_order
└── lib.rs                               # MOD: register PUT /chapters/order

crates/oxipage-ext-projects/src/
├── model.rs                             # MOD: ScreenshotOrderInput
├── repo.rs                              # MOD: reorder_screenshots() in one tx
├── routes.rs                            # MOD: PUT /screenshots/order; validate url; ended_at >= started_at
└── lib.rs                               # MOD: register PUT /screenshots/order

crates/oxipage-ext-blog/src/routes.rs    # MOD: title required, lang must be in settings.site.languages
crates/oxipage-ext-books/src/routes.rs   # MOD: validate ISBN-13 checksum; finished_at >= started_at
crates/oxipage-ext-movies/src/routes.rs  # MOD: release_year 4-digit bounds; positive series_order; media_type guard
crates/oxipage-ext-links/src/routes.rs   # MOD: validate url http(s); thumbnail http(s) or media path; integer order
crates/oxipage-ext-scraps/src/routes.rs  # MOD: source_url http(s); source enum; og_image_url http(s) or media path
```

---

### Task 1: `ApiValidationError` and `jsonOrThrow` field preservation

**Files:**
- Modify: `web/src/admin/shared/api.ts` (top, near existing `OperationConflictError`)

**Interfaces:**
- Consumes: server `ApiError::validation` body `{ error: { code, message, field } }`
- Produces:
  ```ts
  export class ApiValidationError extends Error {
    code: string;
    field: string;
  }
  ```
  and updates `jsonOrThrow<T>` to throw `ApiValidationError` when `body?.error?.field` is present.

- [ ] **Step 1: Add `ApiValidationError` class**

In `web/src/admin/shared/api.ts`, immediately above the existing `OperationConflictError` class, add:

```ts
/// Server validation errors surface as `ApiValidationError`. Carries the
/// offending field so Admin forms can attach the message to the matching
/// DrawerField. Other failures (network, 500) keep their plain Error shape.
export class ApiValidationError extends Error {
  code: string;
  field: string;
  constructor(code: string, field: string, message: string) {
    super(message);
    this.name = "ApiValidationError";
    this.code = code;
    this.field = field;
  }
}
```

- [ ] **Step 2: Update `jsonOrThrow` to preserve field**

Replace the `jsonOrThrow` body in `web/src/admin/shared/api.ts`:

```ts
export async function jsonOrThrow<T>(res: Response): Promise<T> {
  if (!res.ok) {
    const body = await res.json().catch(() => null);
    const detail = body?.error;
    if (detail?.field) {
      throw new ApiValidationError(
        detail.code ?? "validation_error",
        detail.field,
        detail.message ?? "Validation failed",
      );
    }
    const msg = detail?.message ?? detail ?? `HTTP ${res.status}`;
    throw new Error(msg);
  }
  return res.json();
}
```

- [ ] **Step 3: Type-check**

Run: `cd web && npx tsc --noEmit`
Expected: success.

- [ ] **Step 4: Commit**

```bash
git add web/src/admin/shared/api.ts
git commit -m "feat(admin): preserve field-level validation errors in jsonOrThrow"
```

---

### Task 2: Shared client validation helpers

**Files:**
- Create: `web/src/admin/shared/validation.ts`

**Interfaces:**
- Consumes: nothing (pure)
- Produces:
  ```ts
  export function isHttpUrl(v: string): boolean;
  export function isMediaPath(v: string): boolean;
  export function isImageValue(v: string): boolean;
  export function clampRating(v: unknown): { value: number | null; error?: string };
  export function validateYear(v: unknown): { value: number | null; error?: string };
  export function validateDateRange(start: string, end: string): string | null;
  export function validateEmail(v: string): string | null;
  export function validateIsbn13(v: string): string | null;
  ```

- [ ] **Step 1: Create `web/src/admin/shared/validation.ts`**

Create the file:

```ts
// Shared client-side validators. These mirror the server rules in
// crates/oxipage-core/src/validation.rs and the spec's "Validation contract"
// section. Server is authoritative; client feedback is best-effort UX, never
// authoritative.

export function isHttpUrl(v: string): boolean {
  return /^https?:\/\//i.test(v);
}

export function isMediaPath(v: string): boolean {
  if (v.startsWith("/") || v.startsWith(".") || v.includes("..")) return false;
  if (v.startsWith("javascript:") || v.startsWith("data:") || v.startsWith("file:"))
    return false;
  return /^media\/[a-z0-9_-]+\/[a-z0-9._-]+$/i.test(v);
}

export function isImageValue(v: string): boolean {
  return isHttpUrl(v) || isMediaPath(v);
}

export function clampRating(v: unknown): { value: number | null; error?: string } {
  const n = typeof v === "number" ? v : Number(v);
  if (!Number.isFinite(n) || !Number.isInteger(n))
    return { value: null, error: "Rating must be an integer" };
  if (n < 0 || n > 10) return { value: null, error: "Rating must be between 0 and 10" };
  return { value: n };
}

export function validateYear(v: unknown): { value: number | null; error?: string } {
  const n = typeof v === "number" ? v : Number(v);
  if (!Number.isFinite(n) || !Number.isInteger(n))
    return { value: null, error: "Year must be an integer" };
  if (n < 1000 || n > 9999) return { value: null, error: "Year must be a 4-digit value" };
  return { value: n };
}

export function validateDateRange(start: string, end: string): string | null {
  if (!start || !end) return null;
  if (start > end) return "End date must not precede start date";
  return null;
}

export function validateEmail(v: string): string | null {
  if (!v) return null;
  // Pragmatic address check; server re-validates.
  if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(v)) return "Email is not valid";
  return null;
}

/// ISBN-13: 13 digits, last digit is the checksum.
export function validateIsbn13(v: string): string | null {
  if (!v) return null;
  const s = v.replace(/-/g, "");
  if (!/^\d{13}$/.test(s)) return "ISBN-13 must be 13 digits";
  let sum = 0;
  for (let i = 0; i < 12; i++) {
    const d = Number(s[i]);
    sum += i % 2 === 0 ? d : d * 3;
  }
  const check = (10 - (sum % 10)) % 10;
  if (check !== Number(s[12])) return "ISBN-13 checksum is invalid";
  return null;
}
```

- [ ] **Step 2: Type-check**

Run: `cd web && npx tsc --noEmit`
Expected: success.

- [ ] **Step 3: Commit**

```bash
git add web/src/admin/shared/validation.ts
git commit -m "feat(admin): shared client-side validation helpers"
```

---

### Task 3: `TagInput` chip editor

**Files:**
- Create: `web/src/admin/shared/ui/TagInput.tsx`

**Interfaces:**
- Produces:
  ```ts
  interface TagInputProps {
    value: string[];
    onChange: (tags: string[]) => void;
    placeholder?: string;
    error?: string;
  }
  export function TagInput(props: TagInputProps): JSX.Element;
  ```

- [ ] **Step 1: Create `TagInput.tsx`**

Create `web/src/admin/shared/ui/TagInput.tsx`:

```tsx
import { useState, type KeyboardEvent } from "react";
import { X } from "lucide-react";

import { cn } from "../../../shared/ui/cn";

interface TagInputProps {
  value: string[];
  onChange: (tags: string[]) => void;
  placeholder?: string;
  error?: string;
}

/// Chip-based tag editor. Enter or comma adds; Backspace on empty input
/// removes the last tag. onChange returns the new array; never mutates `value`.
export function TagInput({
  value,
  onChange,
  placeholder = "Add a tag…",
  error,
}: TagInputProps) {
  const [draft, setDraft] = useState("");

  const commit = (raw: string) => {
    const tag = raw.trim();
    if (!tag || value.includes(tag)) return;
    onChange([...value, tag]);
    setDraft("");
  };

  const remove = (tag: string) => onChange(value.filter((t) => t !== tag));

  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter" || e.key === ",") {
      e.preventDefault();
      commit(draft);
    } else if (e.key === "Backspace" && draft === "" && value.length > 0) {
      e.preventDefault();
      onChange(value.slice(0, -1));
    }
  };

  return (
    <div>
      <div
        className={cn(
          "flex min-h-10 flex-wrap items-center gap-1.5 rounded-md border bg-canvas px-2 py-1.5",
          error ? "border-red-500" : "border-line",
        )}
      >
        {value.map((tag) => (
          <span
            key={tag}
            className="inline-flex items-center gap-1 rounded-md bg-surface px-2 py-0.5 text-xs text-foreground"
          >
            {tag}
            <button
              type="button"
              onClick={() => remove(tag)}
              aria-label={`Remove ${tag}`}
              className="text-muted hover:text-foreground"
            >
              <X className="size-3" />
            </button>
          </span>
        ))}
        <input
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={onKeyDown}
          onBlur={() => commit(draft)}
          placeholder={value.length === 0 ? placeholder : ""}
          className="flex-1 min-w-[8rem] bg-transparent text-sm text-foreground outline-none placeholder:text-muted"
        />
      </div>
      {error && <p className="text-xs text-red-600 mt-1">{error}</p>}
    </div>
  );
}
```

- [ ] **Step 2: Type-check**

Run: `cd web && npx tsc --noEmit`
Expected: success.

- [ ] **Step 3: Commit**

```bash
git add web/src/admin/shared/ui/TagInput.tsx
git commit -m "feat(admin): TagInput chip editor returning string[]"
```

---

### Task 4: `DrawerField` gains `error` prop

**Files:**
- Modify: `web/src/shared/ui/drawer.tsx`

**Interfaces:**
- Produces: `DrawerFieldProps { label; hint?; required?; error?; children }`

- [ ] **Step 1: Add `error` prop to `DrawerField`**

In `web/src/shared/ui/drawer.tsx`, replace the `DrawerFieldProps` block and function:

```tsx
interface DrawerFieldProps {
  label: string;
  hint?: string;
  required?: boolean;
  error?: string;
  children: React.ReactNode;
}

export function DrawerField({ label, hint, required, error, children }: DrawerFieldProps) {
  return (
    <div className="mb-4">
      <label className="block text-xs font-semibold text-foreground mb-1.5">
        {label}
        {required && <span className="text-red-500 ml-1">*</span>}
      </label>
      {children}
      {hint && !error && <p className="text-xs text-muted mt-1">{hint}</p>}
      {error && <p className="text-xs text-red-600 mt-1">{error}</p>}
    </div>
  );
}
```

- [ ] **Step 2: Type-check**

Run: `cd web && npx tsc --noEmit`
Expected: success.

- [ ] **Step 3: Commit**

```bash
git add web/src/shared/ui/drawer.tsx
git commit -m "feat(ui): DrawerField accepts error prop"
```

---

### Task 5: `EditorPreviewDrawer` — 2-pane desktop, tabs on mobile

**Files:**
- Create: `web/src/admin/shared/ui/EditorPreviewDrawer.tsx`

**Interfaces:**
- Consumes:
  - `AssetResolverProvider` from `web/src/shared/assets` (produced by preview/media plan)
  - `PublicThemeScope` from `web/src/admin/themes/PublicThemeScope` (produced by admin-theme plan)
- Produces:
  ```tsx
  interface EditorPreviewDrawerProps {
    open: boolean;
    onClose: () => void;
    title: string;
    description?: string;
    width?: string;            // desktop drawer width; preview pane fills the rest
    dirty?: boolean;           // close guard
    onRequestClose?: () => boolean | void; // return false to veto close
    editor: React.ReactNode;
    preview: React.ReactNode;
    footer?: React.ReactNode;
  }
  export function EditorPreviewDrawer(props: EditorPreviewDrawerProps): JSX.Element;
  ```

- [ ] **Step 1: Create `EditorPreviewDrawer.tsx`**

Create `web/src/admin/shared/ui/EditorPreviewDrawer.tsx`:

```tsx
import { useEffect, useState } from "react";
import { X } from "lucide-react";

import { cn } from "../../../shared/ui/cn";
import { AssetResolverProvider } from "../../../shared/assets"; // produced by preview/media plan
import { PublicThemeScope } from "../../themes/PublicThemeScope"; // produced by admin-theme plan

interface EditorPreviewDrawerProps {
  open: boolean;
  onClose: () => void;
  title: string;
  description?: string;
  width?: string; // tailwind class for the editor pane on desktop
  dirty?: boolean;
  onRequestClose?: () => boolean | void;
  editor: React.ReactNode;
  preview: React.ReactNode;
  footer?: React.ReactNode;
}

/// 2-pane editor + preview on desktop (≥ md). Smaller viewports collapse to
/// Edit/Preview tabs. The preview pane is wrapped in the site's PublicTheme
/// and the site's admin asset resolver so media URLs resolve identically to
/// the published site.
export function EditorPreviewDrawer({
  open,
  onClose,
  title,
  description,
  width = "w-[460px]",
  dirty = false,
  onRequestClose,
  editor,
  preview,
  footer,
}: EditorPreviewDrawerProps) {
  const [tab, setTab] = useState<"edit" | "preview">("edit");

  // Reset to Edit whenever the drawer opens.
  useEffect(() => {
    if (open) setTab("edit");
  }, [open]);

  // Escape + outside click close hook: route through dirty/confirm guard.
  useEffect(() => {
    if (!open) return;
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") attemptClose();
    }
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, dirty]);

  function attemptClose() {
    if (dirty && !window.confirm("Discard unsaved changes?")) return;
    if (onRequestClose && onRequestClose() === false) return;
    onClose();
  }

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-50 flex bg-black/40">
      <div className="flex-1" onClick={attemptClose} aria-hidden />

      {/* Desktop: 2-pane. Mobile: single pane with tabs. */}
      <div
        className={cn(
          "bg-canvas border-l border-line h-full overflow-hidden flex flex-col shadow-2xl",
          width,
        )}
        role="dialog"
        aria-modal="true"
        aria-label={title}
      >
        <header className="flex items-start justify-between px-5 py-4 border-b border-line">
          <div>
            <h2 className="text-base font-semibold text-foreground">{title}</h2>
            {description && <p className="text-xs text-muted mt-0.5">{description}</p>}
          </div>
          <button
            onClick={attemptClose}
            className="inline-flex items-center justify-center size-7 rounded-md text-muted hover:text-foreground hover:bg-surface/50"
            aria-label="Close"
          >
            <X size={16} />
          </button>
        </header>

        {/* Mobile tabs (hidden md:flex). */}
        <div className="flex border-b border-line md:hidden">
          {(["edit", "preview"] as const).map((id) => (
            <button
              key={id}
              onClick={() => setTab(id)}
              className={cn(
                "flex-1 px-4 py-2 text-sm font-medium capitalize",
                tab === id ? "text-foreground border-b-2 border-[#22c55e]" : "text-muted",
              )}
            >
              {id}
            </button>
          ))}
        </div>

        <div className="flex-1 min-h-0 flex">
          {/* Editor pane: hidden on mobile when Preview tab is active. */}
          <div
            className={cn(
              "overflow-y-auto px-5 py-4",
              "w-full md:w-[var(--editor-w,460px)] md:shrink-0 md:border-r md:border-line",
              tab === "preview" ? "hidden md:block" : "block",
            )}
            style={{ ["--editor-w" as never]: undefined }}
          >
            {editor}
          </div>

          {/* Preview pane: scoped to the site's public theme + admin resolver. */}
          <div
            className={cn(
              "flex-1 min-w-0 overflow-y-auto bg-surface/40",
              tab === "edit" ? "hidden md:block" : "block",
            )}
          >
            <PublicThemeScope>
              <AssetResolverProvider mode="admin">{preview}</AssetResolverProvider>
            </PublicThemeScope>
          </div>
        </div>

        {footer && (
          <div className="px-5 py-4 border-t border-line flex justify-end gap-2 bg-surface/30">
            {footer}
          </div>
        )}
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Type-check**

Run: `cd web && npx tsc --noEmit`
Expected: success.

- [ ] **Step 3: Commit**

```bash
git add web/src/admin/shared/ui/EditorPreviewDrawer.tsx
git commit -m "feat(admin): EditorPreviewDrawer with 2-pane desktop and mobile tabs"
```

---

### Task 6: `DraftPreviewPane` — render a *View from local form state

**Files:**
- Create: `web/src/admin/shared/ui/DraftPreviewPane.tsx`

**Interfaces:**
- Produces:
  ```tsx
  interface DraftPreviewPaneProps {
    children: React.ReactNode; // *View element bound to local form state
  }
  export function DraftPreviewPane(props: DraftPreviewPaneProps): JSX.Element;
  ```
  Renders within a card surface with a "Draft Preview" header so authors never confuse it with `Preview Site` (the last static build).

- [ ] **Step 1: Create `DraftPreviewPane.tsx`**

Create `web/src/admin/shared/ui/DraftPreviewPane.tsx`:

```tsx
import type { ReactNode } from "react";

/// Wraps a presentation component (e.g. BlogPostView) rendered from local
/// form state. Adds a header that distinguishes "Draft Preview" from
/// "Preview Site" (the last static build of the site).
export function DraftPreviewPane({ children }: { children: ReactNode }) {
  return (
    <div className="rounded-md border border-line bg-canvas">
      <div className="flex items-center justify-between border-b border-line px-4 py-2 text-xs text-muted">
        <span className="font-medium">Draft Preview</span>
        <span>unsaved local state</span>
      </div>
      <div className="p-4">{children}</div>
    </div>
  );
}
```

- [ ] **Step 2: Type-check**

Run: `cd web && npx tsc --noEmit`
Expected: success.

- [ ] **Step 3: Commit**

```bash
git add web/src/admin/shared/ui/DraftPreviewPane.tsx
git commit -m "feat(admin): DraftPreviewPane distinguishes from Preview Site"
```

---

### Task 7: `ImageField` — surface `error` prop

**Files:**
- Modify: `web/src/admin/shared/ui/ImageField.tsx`

This file is produced by the preview/media plan. We add a thin error display to it. If the symbol is not yet present, leave this task to execute after the peer plan and skip to Task 8.

**Interfaces:**
- Produces: existing `ImageField` props gain optional `error?: string`.

- [ ] **Step 1: Confirm `ImageField.tsx` exists**

Run: `ls web/src/admin/shared/ui/ImageField.tsx`
Expected: file present (produced by peer plan).

- [ ] **Step 2: Add `error` prop and render**

If `ImageField` exists, modify its props type and JSX to accept and display an error. Exact diff depends on the file produced by the peer plan; the minimum change is:

```tsx
interface ImageFieldProps {
  // ...existing props...
  error?: string;
}
// inside the component JSX, render {error && <p className="text-xs text-red-600 mt-1">{error}</p>}
```

If the peer plan already includes `error`, no edit is needed.

- [ ] **Step 3: Type-check**

Run: `cd web && npx tsc --noEmit`
Expected: success.

- [ ] **Step 4: Commit**

```bash
git add web/src/admin/shared/ui/ImageField.tsx
git commit -m "feat(admin): ImageField surfaces field-level error"
```

---

### Task 8: Split `BlogPostPage` → `BlogPostView`

**Files:**
- Create: `web/src/extensions/blog/BlogPostView.tsx`
- Create: `web/src/extensions/blog/BlogPostCard.tsx`
- Modify: `web/src/extensions/blog/BlogPostPage.tsx` — fetch only, render `<BlogPostView post language />`
- Modify: `web/src/extensions/blog/BlogListPage.tsx` — render `<BlogPostCard post={p} />`

**Interfaces:**
- Produces:
  ```tsx
  // BlogPostView.tsx
  export interface BlogPostData {
    title: string;
    body: string;
    lang: "ko" | "en";
    tags: string[];
    published_at: string | null;
    created_at: string;
  }
  interface BlogPostViewProps {
    post: BlogPostData;
    language: "ko" | "en";
  }
  export function BlogPostView(props: BlogPostViewProps): JSX.Element;
  ```

- [ ] **Step 1: Create `BlogPostView.tsx`**

Create `web/src/extensions/blog/BlogPostView.tsx`:

```tsx
import { Calendar } from "lucide-react";

import { Markdown } from "../../shared/Markdown";
import { Badge } from "../../shared/ui/badge";
import { Card, CardContent } from "../../shared/ui/card";

export interface BlogPostData {
  title: string;
  body: string;
  lang: "ko" | "en";
  tags: string[];
  published_at: string | null;
  created_at: string;
}

interface BlogPostViewProps {
  post: BlogPostData;
  language: "ko" | "en";
}

export function BlogPostView({ post, language: _language }: BlogPostViewProps) {
  const date = (post.published_at ?? post.created_at).slice(0, 10);
  return (
    <article className="space-y-6">
      <header className="space-y-3">
        <h1 className="font-serif text-3xl font-semibold tracking-tight text-foreground">
          {post.title}
        </h1>
        <div className="flex flex-wrap items-center gap-x-3 gap-y-1.5 text-sm text-subtle">
          <span className="inline-flex items-center gap-1">
            <Calendar className="size-3.5" />
            <time>{date}</time>
          </span>
          <Badge variant="outline">{post.lang === "ko" ? "한국어" : "English"}</Badge>
          {post.tags.length > 0 &&
            post.tags.map((t) => (
              <Badge key={t} variant="secondary">
                {t}
              </Badge>
            ))}
        </div>
      </header>
      <Card>
        <CardContent className="markdown pt-6">
          <Markdown source={post.body || "*No content*"} />
        </CardContent>
      </Card>
    </article>
  );
}
```

- [ ] **Step 2: Replace `BlogPostPage.tsx` body**

In `web/src/extensions/blog/BlogPostPage.tsx`, replace the file:

```tsx
import { useQuery } from "@tanstack/react-query";
import { ArrowLeft } from "lucide-react";
import { Link, useParams } from "react-router";

import { fetchBlogPost } from "../../shared/api";
import { useLanguage } from "../../shared/language";
import { Button } from "../../shared/ui/button";
import { BlogPostView } from "./BlogPostView";

export function BlogPostPage() {
  const { slug = "" } = useParams();
  const { lang } = useLanguage();
  const { data: post, isLoading, error } = useQuery({
    queryKey: ["blog", slug],
    queryFn: () => fetchBlogPost(slug),
    enabled: !!slug,
  });

  if (isLoading) return <p className="text-subtle">…</p>;
  if (error || !post) {
    return (
      <div className="space-y-4">
        <Button variant="ghost" size="sm" asChild>
          <Link to="/blog">
            <ArrowLeft />
            {lang === "ko" ? "블로그" : "Blog"}
          </Link>
        </Button>
        <p className="text-subtle">
          {lang === "ko" ? "게시물을 찾을 수 없습니다." : "Post not found."}
        </p>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <Button variant="ghost" size="sm" asChild className="-ml-2">
        <Link to="/blog">
          <ArrowLeft />
          {lang === "ko" ? "블로그" : "Blog"}
        </Link>
      </Button>
      <BlogPostView post={post} language={lang} />
    </div>
  );
}
```

- [ ] **Step 3: Create `BlogPostCard` and reuse in `BlogListPage`**

Create `web/src/extensions/blog/BlogPostCard.tsx`:

```tsx
import { Link } from "react-router";

import { Card } from "../../shared/ui/card";

export interface BlogPostCardData {
  slug: string;
  title: string;
  tags: string[];
  published_at: string | null;
}

export function BlogPostCard({ post }: { post: BlogPostCardData }) {
  return (
    <Card className="transition-[border-color,box-shadow] duration-200 hover:border-primary/40 hover:shadow-md">
      <Link to={`/blog/${post.slug}`} className="block p-5 text-foreground no-underline">
        <h2 className="font-serif text-xl font-semibold tracking-tight">{post.title}</h2>
        {post.tags.length > 0 && (
          <p className="mt-2 text-xs text-muted">#{post.tags.join(" #")}</p>
        )}
      </Link>
    </Card>
  );
}
```

In `web/src/extensions/blog/BlogListPage.tsx`, replace the `<Card>…</Card>` mapping inside `.map(...)`:

```tsx
import { BlogPostCard } from "./BlogPostCard";
// inside .map:
<BlogPostCard key={p.slug} post={{ slug: p.slug, title: p.title, tags: p.tags, published_at: p.published_at }} />
```

- [ ] **Step 4: Type-check**

Run: `cd web && npx tsc --noEmit`
Expected: success.

- [ ] **Step 5: Smoke**

Run: `cd web && bun run dev`
Visit `/blog`, click a post, verify title, body, tags render identically.

- [ ] **Step 6: Commit**

```bash
git add web/src/extensions/blog/
git commit -m "refactor(blog): extract BlogPostView and BlogPostCard"
```

---

### Task 9: Split `ProfilePage` → `ProfileView`

**Files:**
- Create: `web/src/extensions/profile/ProfileView.tsx`
- Modify: `web/src/extensions/profile/ProfilePage.tsx` — fetch only, render `<ProfileView profile language />`

**Interfaces:**
- Produces:
  ```tsx
  export interface ProfileData { ... }   // mirror of crates/.../model.rs::Profile
  interface ProfileViewProps { profile: ProfileData; language: "ko" | "en"; }
  ```

- [ ] **Step 1: Create `ProfileView.tsx`**

Create `web/src/extensions/profile/ProfileView.tsx`:

```tsx
import { Briefcase, Code, Globe, Mail } from "lucide-react";

import { Markdown } from "../../shared/Markdown";
import { Card, CardContent } from "../../shared/ui/card";

export interface ProfileEducation {
  institution: string | null;
  degree: string | null;
  field: string | null;
  start_year: number | null;
  end_year: number | null;
}

export interface ProfileCustomLink {
  label: string;
  url: string;
  icon?: string | null;
}

export interface ProfileData {
  display_name: string;
  tagline_ko: string | null;
  tagline_en: string | null;
  avatar_url: string | null;
  bio_ko: string | null;
  bio_en: string | null;
  email: string | null;
  github_username: string | null;
  linkedin_url: string | null;
  education: ProfileEducation[];
  custom_links: ProfileCustomLink[];
  updated_at: string;
}

interface ProfileViewProps {
  profile: ProfileData;
  language: "ko" | "en";
}

function pick<T>(lang: "ko" | "en", ko: T | null, en: T | null): T | null {
  return lang === "ko" ? ko ?? en : en ?? ko;
}

export function ProfileView({ profile, language }: ProfileViewProps) {
  const tagline = pick(language, profile.tagline_ko, profile.tagline_en);
  const bio = pick(language, profile.bio_ko, profile.bio_en);
  const avatarUrl = profile.avatar_url;

  return (
    <article className="space-y-6">
      <Card>
        <div className="flex flex-col gap-5 p-6 sm:flex-row sm:items-start">
          {avatarUrl && (
            <img
              src={avatarUrl}
              alt={profile.display_name}
              className="size-20 shrink-0 rounded-full border border-line object-cover"
            />
          )}
          <div className="min-w-0 space-y-1.5">
            <h1 className="font-serif text-2xl font-semibold tracking-tight text-foreground">
              {profile.display_name}
            </h1>
            {tagline && <p className="leading-relaxed text-muted">{tagline}</p>}
            <nav className="flex flex-wrap gap-x-4 gap-y-1.5 pt-2 text-sm">
              {profile.email && (
                <a className="inline-flex items-center gap-1.5 text-muted hover:text-primary" href={`mailto:${profile.email}`}>
                  <Mail className="size-3.5" />
                  {profile.email}
                </a>
              )}
              {profile.github_username && (
                <a className="inline-flex items-center gap-1.5 text-muted hover:text-primary" href={`https://github.com/${profile.github_username}`} rel="me">
                  <Code className="size-3.5" />
                  GitHub
                </a>
              )}
              {profile.linkedin_url && (
                <a className="inline-flex items-center gap-1.5 text-muted hover:text-primary" href={profile.linkedin_url}>
                  <Briefcase className="size-3.5" />
                  LinkedIn
                </a>
              )}
              {profile.custom_links.map((l) => (
                <a key={l.url} className="inline-flex items-center gap-1.5 text-muted hover:text-primary" href={l.url}>
                  <Globe className="size-3.5" />
                  {l.label}
                </a>
              ))}
            </nav>
          </div>
        </div>
      </Card>

      {bio && (
        <Card>
          <CardContent className="markdown pt-6">
            <Markdown source={bio} />
          </CardContent>
        </Card>
      )}

      {profile.education.length > 0 && (
        <section className="space-y-3">
          <h2 className="font-serif text-xl font-semibold tracking-tight text-foreground">
            {language === "ko" ? "학력" : "Education"}
          </h2>
          <ul className="space-y-2">
            {profile.education.map((e, i) => (
              <li key={i}>
                <Card className="px-4 py-3 shadow-xs">
                  <span className="font-medium text-foreground">{e.institution}</span>
                  {(e.degree || e.field) && (
                    <span className="text-muted">
                      {" "}— {[e.degree, e.field].filter(Boolean).join(", ")}
                    </span>
                  )}
                  {(e.start_year || e.end_year) && (
                    <span className="text-subtle">
                      {" "}({e.start_year ?? "?"}–{e.end_year ?? (language === "ko" ? "현재" : "present")})
                    </span>
                  )}
                </Card>
              </li>
            ))}
          </ul>
        </section>
      )}
    </article>
  );
}
```

- [ ] **Step 2: Replace `ProfilePage.tsx`**

In `web/src/extensions/profile/ProfilePage.tsx`, replace the file:

```tsx
import { useQuery } from "@tanstack/react-query";

import { fetchProfile, type ProfileData } from "../../shared/api";
import { useLanguage } from "../../shared/language";
import { ProfileView } from "./ProfileView";

export function ProfilePage() {
  const { lang } = useLanguage();
  const { data: profile, isLoading, error } = useQuery<ProfileData | null>({
    queryKey: ["profile"],
    queryFn: fetchProfile,
  });

  if (isLoading) return <p className="text-subtle">…</p>;
  if (error || !profile) {
    return (
      <p className="text-subtle">
        {lang === "ko" ? "프로필을 불러오지 못했습니다." : "Failed to load profile."}
      </p>
    );
  }

  return <ProfileView profile={profile} language={lang} />;
}
```

(If `fetchProfile` doesn't yet return `ProfileData` (the `updated_at` field), update its typing in `web/src/shared/api.ts` accordingly.)

- [ ] **Step 3: Type-check and smoke**

Run: `cd web && npx tsc --noEmit`
Then `cd web && bun run dev` and visit `/profile`; verify all sections render identically to pre-refactor.

- [ ] **Step 4: Commit**

```bash
git add web/src/extensions/profile/
git commit -m "refactor(profile): extract ProfileView presentation"
```

---

### Task 10: Extract `BookCard`, `MovieCard`, `NovelCard`, `LinkCard`, `ScrapCard`, `ProjectCard` + `ProjectView`

**Files:**
- Create:
  - `web/src/extensions/books/BookCard.tsx`
  - `web/src/extensions/movies/MovieCard.tsx`
  - `web/src/extensions/novels/NovelCard.tsx`
  - `web/src/extensions/links/LinkCard.tsx`
  - `web/src/extensions/scraps/ScrapCard.tsx`
  - `web/src/extensions/projects/ProjectCard.tsx`
  - `web/src/extensions/projects/ProjectView.tsx`
- Modify: each list `*Page.tsx` to render the corresponding card.

**Interfaces:**
- Each card is pure: typed props, no fetch, no toast. `ProjectView` mirrors `ProjectDetailPage` body.

- [ ] **Step 1: Create `BookCard.tsx`**

Create `web/src/extensions/books/BookCard.tsx`:

```tsx
import { BookOpen } from "lucide-react";

import { Card } from "../../shared/ui/card";
import { Badge } from "../../shared/ui/badge";
import { RatingStars } from "../../shared/RatingStars";

export interface BookCardData {
  id: number;
  title: string;
  author: string | null;
  cover_image_url: string | null;
  rating: number;
  review_ko: string | null;
  review_en: string | null;
  status: string;
}

interface BookCardProps {
  book: BookCardData;
  pick: <T,>(ko: T | null, en: T | null) => T | null;
}

const STATUS_LABEL: Record<string, { ko: string; en: string }> = {
  wishlist: { ko: "읽고 싶음", en: "Wishlist" },
  reading: { ko: "읽는중", en: "Reading" },
  completed: { ko: "완독", en: "Completed" },
  dropped: { ko: "중단", en: "Dropped" },
};

export function BookCard({ book, pick }: BookCardProps) {
  const status = STATUS_LABEL[book.status] ?? { ko: book.status, en: book.status };
  return (
    <Card className="flex h-full gap-4 p-4">
      {book.cover_image_url ? (
        <img src={book.cover_image_url} alt="" className="w-14 shrink-0 rounded-md object-cover" loading="lazy" />
      ) : (
        <div className="flex w-14 shrink-0 items-center justify-center rounded-md bg-surface text-subtle">
          <BookOpen className="size-5" />
        </div>
      )}
      <div className="min-w-0 space-y-1">
        <h2 className="font-serif text-base font-semibold leading-tight text-foreground">{book.title}</h2>
        {book.author && <p className="text-xs text-subtle">{book.author}</p>}
        <div className="flex items-center gap-2">
          <RatingStars value={book.rating} size="sm" />
          <Badge variant="secondary">{pick(status.ko, status.en) ?? status.en}</Badge>
        </div>
        {pick(book.review_ko, book.review_en) && (
          <p className="line-clamp-3 text-sm text-subtle">{pick(book.review_ko, book.review_en)}</p>
        )}
      </div>
    </Card>
  );
}
```

- [ ] **Step 2: Replace card mapping in `BooksPage.tsx`**

In `web/src/extensions/books/BooksPage.tsx`, replace the `<li>...<Card>...</Card></li>` body of `books.map((b) => …)`:

```tsx
import { BookCard } from "./BookCard";
// inside .map:
<li key={b.id}>
  <BookCard book={b} pick={pick} />
</li>
```

- [ ] **Step 3: Create `MovieCard.tsx`**

Create `web/src/extensions/movies/MovieCard.tsx`:

```tsx
import { Film } from "lucide-react";

import { Card } from "../../shared/ui/card";
import { RatingStars } from "../../shared/RatingStars";

export interface MovieCardData {
  id: number;
  title: string;
  media_type: "movie" | "tv";
  poster_path: string | null;
  release_year: number | null;
  rating: number;
  review_ko: string | null;
  review_en: string | null;
}

function posterUrl(path: string | null) {
  return path ? `https://image.tmdb.org/t/p/w200${path}` : null;
}

interface MovieCardProps {
  movie: MovieCardData;
  pick: <T,>(ko: T | null, en: T | null) => T | null;
}

export function MovieCard({ movie, pick }: MovieCardProps) {
  const img = posterUrl(movie.poster_path);
  return (
    <Card className="flex h-full gap-4 p-4">
      {img ? (
        <img src={img} alt="" className="w-14 shrink-0 rounded-md object-cover" loading="lazy" />
      ) : (
        <div className="flex w-14 shrink-0 items-center justify-center rounded-md bg-surface text-subtle">
          <Film className="size-5" />
        </div>
      )}
      <div className="min-w-0 space-y-1">
        <h2 className="font-serif text-base font-semibold leading-tight text-foreground">{movie.title}</h2>
        <div className="flex items-center gap-2 text-xs text-subtle">
          <span className="uppercase">{movie.media_type}</span>
          {movie.release_year && <span>· {movie.release_year}</span>}
        </div>
        <RatingStars value={movie.rating} size="sm" />
        {pick(movie.review_ko, movie.review_en) && (
          <p className="line-clamp-3 text-sm text-subtle">{pick(movie.review_ko, movie.review_en)}</p>
        )}
      </div>
    </Card>
  );
}
```

- [ ] **Step 4: Replace card mapping in `MoviesPage.tsx`**

In `web/src/extensions/movies/MoviesPage.tsx`, replace the `.map(...)` body:

```tsx
import { MovieCard } from "./MovieCard";
// inside .map:
<li key={m.id}>
  <MovieCard movie={m} pick={pick} />
</li>
```

- [ ] **Step 5: Create `NovelCard.tsx`**

Create `web/src/extensions/novels/NovelCard.tsx`:

```tsx
import { Card } from "../../shared/ui/card";
import { Badge } from "../../shared/ui/badge";

export interface NovelCardData {
  id: number;
  title: string;
  synopsis: string | null;
  cover_image: string | null;
  status: string;
  tags: string[];
}

const STATUS_LABEL: Record<string, { ko: string; en: string }> = {
  ongoing: { ko: "연재중", en: "Ongoing" },
  completed: { ko: "완결", en: "Completed" },
  hiatus: { ko: "휴재", en: "Hiatus" },
};

interface NovelCardProps {
  novel: NovelCardData;
  pick: <T,>(ko: T | null, en: T | null) => T | null;
}

export function NovelCard({ novel, pick }: NovelCardProps) {
  const status = STATUS_LABEL[novel.status] ?? { ko: novel.status, en: novel.status };
  return (
    <Card className="flex h-full gap-4 p-4">
      {novel.cover_image && (
        <img src={novel.cover_image} alt="" className="size-20 shrink-0 rounded-md object-cover" loading="lazy" />
      )}
      <div className="min-w-0 space-y-1">
        <h2 className="font-serif text-base font-semibold leading-tight text-foreground">{novel.title}</h2>
        <Badge variant="secondary">{pick(status.ko, status.en) ?? status.en}</Badge>
        {novel.synopsis && <p className="line-clamp-3 text-sm text-subtle">{novel.synopsis}</p>}
        {novel.tags.length > 0 && <p className="text-xs text-subtle">#{novel.tags.join(" #")}</p>}
      </div>
    </Card>
  );
}
```

- [ ] **Step 6: Replace card mapping in `NovelsPage.tsx`**

In `web/src/extensions/novels/NovelsPage.tsx`, replace the `.map(...)` body:

```tsx
import { NovelCard } from "./NovelCard";
// inside .map:
<li key={n.id}>
  <NovelCard novel={n} pick={pick} />
</li>
```

- [ ] **Step 7: Create `LinkCard.tsx`**

Create `web/src/extensions/links/LinkCard.tsx`:

```tsx
import { ExternalLink, Star } from "lucide-react";

import { Card } from "../../shared/ui/card";

export interface LinkCardData {
  id: number;
  url: string;
  title: string;
  description_ko: string | null;
  description_en: string | null;
  thumbnail_url: string | null;
  featured: boolean;
}

interface LinkCardProps {
  link: LinkCardData;
  pick: <T,>(ko: T | null, en: T | null) => T | null;
}

function safeHost(url: string) {
  try { return new URL(url).host; } catch { return url; }
}

export function LinkCard({ link, pick }: LinkCardProps) {
  const description = pick(link.description_ko, link.description_en);
  return (
    <li className="relative">
      {link.featured && (
        <Star className="absolute right-3 top-3 z-10 size-4 fill-star text-star" />
      )}
      <Card className={"h-full transition-[border-color,box-shadow] duration-200 hover:border-primary/40 hover:shadow-md " + (link.featured ? "border-primary/50 " : "")}>
        <a href={link.url} rel="noreferrer noopener" className="flex h-full gap-3 p-4 text-foreground no-underline">
          {link.thumbnail_url && (
            <img src={link.thumbnail_url} alt="" loading="lazy" className="size-16 shrink-0 rounded-md border border-line object-cover" />
          )}
          <div className="min-w-0 flex-1">
            <h2 className="truncate font-medium text-foreground">{link.title}</h2>
            {description && <p className="mt-0.5 line-clamp-2 text-sm text-muted">{description}</p>}
            <span className="mt-1 inline-flex items-center gap-1 text-xs text-subtle">
              <ExternalLink className="size-3" />
              {safeHost(link.url)}
            </span>
          </div>
        </a>
      </Card>
    </li>
  );
}
```

- [ ] **Step 8: Replace card mapping in `LinksPage.tsx`**

In `web/src/extensions/links/LinksPage.tsx`, replace the `<li>...<Card>...</Card></li>` body inside `.map(...)`:

```tsx
import { LinkCard } from "./LinkCard";
// inside .map (return only LinkCard, drop <li> wrapper since LinkCard already renders <li>):
<LinkCard key={l.id} link={l} pick={pick} />
```

- [ ] **Step 9: Create `ScrapCard.tsx`**

Create `web/src/extensions/scraps/ScrapCard.tsx`:

```tsx
import { ExternalLink } from "lucide-react";

import { Card } from "../../shared/ui/card";
import { Badge } from "../../shared/ui/badge";

export interface ScrapCardData {
  id: number;
  title: string;
  source_url: string;
  og_image_url: string | null;
  note_ko: string | null;
  note_en: string | null;
  source: string;
  tags: string[];
}

function safeHost(url: string) {
  try { return new URL(url).host; } catch { return url; }
}

interface ScrapCardProps {
  scrap: ScrapCardData;
  pick: <T,>(ko: T | null, en: T | null) => T | null;
}

export function ScrapCard({ scrap, pick }: ScrapCardProps) {
  return (
    <Card className="flex h-full flex-col gap-3 p-4">
      <div className="flex items-start gap-3">
        {scrap.og_image_url ? (
          <img src={scrap.og_image_url} alt="" className="size-12 shrink-0 rounded-md object-cover" loading="lazy" />
        ) : (
          <div className="flex size-12 shrink-0 items-center justify-center rounded-md bg-surface text-subtle">
            <ExternalLink className="size-4" />
          </div>
        )}
        <div className="min-w-0 space-y-1">
          <a href={scrap.source_url} target="_blank" rel="noopener noreferrer" className="font-serif text-base font-semibold leading-tight text-foreground hover:text-primary">
            {scrap.title}
          </a>
          <p className="text-xs text-subtle">{safeHost(scrap.source_url)}</p>
        </div>
      </div>
      {pick(scrap.note_ko, scrap.note_en) && (
        <p className="line-clamp-3 text-sm text-subtle">{pick(scrap.note_ko, scrap.note_en)}</p>
      )}
      <div className="mt-auto flex items-center gap-2">
        <Badge variant="secondary">{scrap.source}</Badge>
        {scrap.tags.length > 0 && <span className="text-xs text-subtle">#{scrap.tags.join(" #")}</span>}
      </div>
    </Card>
  );
}
```

- [ ] **Step 10: Replace card mapping in `ScrapsPage.tsx`**

In `web/src/extensions/scraps/ScrapsPage.tsx`, replace the `.map(...)` body:

```tsx
import { ScrapCard } from "./ScrapCard";
// inside .map:
<ScrapCard key={s.id} scrap={s} pick={pick} />
```

- [ ] **Step 11: Create `ProjectCard.tsx` and `ProjectView.tsx`**

Create `web/src/extensions/projects/ProjectCard.tsx`:

```tsx
import { Link } from "react-router";
import { Star } from "lucide-react";

import { Card } from "../../shared/ui/card";
import { Badge } from "../../shared/ui/badge";

export interface ProjectCardData {
  slug: string;
  title_ko: string | null;
  title_en: string | null;
  tech_stack: string[];
  status: string;
  featured: boolean;
}

interface ProjectCardProps {
  project: ProjectCardData;
  pick: <T,>(ko: T | null, en: T | null) => T | null;
}

export function ProjectCard({ project, pick }: ProjectCardProps) {
  const title = pick(project.title_ko, project.title_en) ?? project.slug;
  return (
    <li className="relative">
      {project.featured && (
        <Star className="absolute right-3 top-3 z-10 size-4 fill-star text-star" />
      )}
      <Card className={"h-full transition-[border-color,box-shadow] duration-200 hover:border-primary/40 hover:shadow-md " + (project.featured ? "border-primary/50 " : "")}>
        <Link to={`/projects/${project.slug}`} className="block h-full p-5 text-foreground no-underline">
          <h2 className="font-serif text-lg font-semibold tracking-tight">{title}</h2>
          <div className="mt-2">
            <Badge variant="secondary">{project.status}</Badge>
          </div>
          {project.tech_stack.length > 0 && (
            <p className="mt-2 text-sm text-subtle">{project.tech_stack.join(" · ")}</p>
          )}
        </Link>
      </Card>
    </li>
  );
}
```

Create `web/src/extensions/projects/ProjectView.tsx`:

```tsx
import { ExternalLink } from "lucide-react";

import { Markdown } from "../../shared/Markdown";
import { Badge } from "../../shared/ui/badge";
import { Button } from "../../shared/ui/button";
import { Card, CardContent } from "../../shared/ui/card";

export interface ProjectScreenshot {
  id: number;
  url: string;
  alt_ko: string | null;
  alt_en: string | null;
}

interface ProjectLinks {
  repo?: string;
  demo?: string;
  app_store?: string;
  play_store?: string;
  custom?: { label: string; url: string }[];
}

export interface ProjectViewData {
  title_ko: string | null;
  title_en: string | null;
  description_ko: string | null;
  description_en: string | null;
  tech_stack: string[];
  status: string;
  started_at: string | null;
  ended_at: string | null;
  links: ProjectLinks | unknown;
  screenshots: ProjectScreenshot[];
}

interface ProjectViewProps {
  project: ProjectViewData;
  pick: <T,>(ko: T | null, en: T | null) => T | null;
}

const STATUS_VARIANT: Record<string, "positive" | "accent" | "secondary"> = {
  active: "positive",
  shipped: "positive",
  wip: "accent",
  planning: "secondary",
  paused: "secondary",
  archived: "secondary",
};

export function ProjectView({ project, pick }: ProjectViewProps) {
  const title = pick(project.title_ko, project.title_en) ?? "";
  const description = pick(project.description_ko, project.description_en);
  const links = (project.links ?? {}) as ProjectLinks;
  const linkEntries = [
    links.repo && { label: "Repo", url: links.repo },
    links.demo && { label: "Demo", url: links.demo },
    links.app_store && { label: "App Store", url: links.app_store },
    links.play_store && { label: "Play Store", url: links.play_store },
    ...(links.custom ?? []),
  ].filter((x): x is { label: string; url: string } => !!x);

  return (
    <article className="space-y-6">
      <Card>
        <div className="space-y-3 p-6">
          <h1 className="font-serif text-3xl font-semibold tracking-tight text-foreground">{title}</h1>
          <div className="flex flex-wrap items-center gap-2 text-sm text-subtle">
            <Badge variant={STATUS_VARIANT[project.status] ?? "secondary"}>{project.status}</Badge>
            {project.tech_stack.length > 0 && <span>{project.tech_stack.join(" · ")}</span>}
          </div>
          {(project.started_at || project.ended_at) && (
            <p className="text-xs text-subtle">
              {project.started_at ?? "?"} – {project.ended_at ?? "present"}
            </p>
          )}
          {linkEntries.length > 0 && (
            <nav className="flex flex-wrap gap-2 pt-1">
              {linkEntries.map((l) => (
                <Button key={l.url} variant="secondary" size="sm" asChild>
                  <a href={l.url} rel="noreferrer noopener"><ExternalLink />{l.label}</a>
                </Button>
              ))}
            </nav>
          )}
        </div>
      </Card>

      {description && (
        <Card>
          <CardContent className="markdown pt-6"><Markdown source={description} /></CardContent>
        </Card>
      )}

      {project.screenshots.length > 0 && (
        <section className="grid gap-4 sm:grid-cols-2">
          {project.screenshots.map((s) => (
            <figure key={s.id} className="overflow-hidden rounded-lg border border-line bg-surface shadow-sm">
              <img src={s.url} alt={pick(s.alt_ko, s.alt_en) ?? ""} loading="lazy" className="block w-full" />
            </figure>
          ))}
        </section>
      )}
    </article>
  );
}
```

- [ ] **Step 12: Replace cards in `ProjectsListPage.tsx` and body in `ProjectDetailPage.tsx`**

In `web/src/extensions/projects/ProjectsListPage.tsx`, replace the `<li>...<Card>...</Card></li>` body:

```tsx
import { ProjectCard } from "./ProjectCard";
// inside .map:
<ProjectCard key={p.slug} project={p} pick={pick} />
```

In `web/src/extensions/projects/ProjectDetailPage.tsx`, replace the `<article>` body (keep ArrowLeft + Back link, but render `ProjectView` below):

```tsx
import { ProjectView } from "./ProjectView";
// inside the component after the back link:
<ProjectView project={project} pick={pick} />
```

- [ ] **Step 13: Type-check and smoke**

Run: `cd web && npx tsc --noEmit` and `cd web && bun run dev`; visit each list and detail page and confirm rendering matches the prior inline JSX exactly.

- [ ] **Step 14: Commit**

```bash
git add web/src/extensions/
git commit -m "refactor: extract *Card and ProjectView presentation components"
```

---

### Task 11: Profile CRUD APIs (get/upsert with optimistic concurrency)

**Files:**
- Modify: `crates/oxipage-ext-profile/src/model.rs`
- Modify: `crates/oxipage-ext-profile/src/repo.rs`
- Modify: `crates/oxipage-ext-profile/src/routes.rs`

 **Interfaces:**
 - Produces:
   ```rust
   pub struct ProfileInput {
       pub expected_updated_at: String, // ISO timestamp from prior GET; "" for first write
       pub display_name: String,
       // ... existing fields
   }
   pub enum UpsertError { Stale { expected: String }, Db(anyhow::Error) }
   pub async fn upsert_if_unchanged(pool, input) -> Result<Profile, UpsertError>;
   // On stale detection the 409 response body is:
   //   { error: { code: "stale_profile", message, field: null },
   //     data:   <current Profile> }
   // `ApiError::with_data(status, code, message, &data)` is added so the
   // Profile route can attach the remote row to the 409 body. `ErrorBody`
   // gains an optional `data: Option<serde_json::Value>` (skip-if-none).
   ```

- [ ] **Step 0: Extend `ErrorBody` and add `ApiError::with_data`**

In `crates/oxipage-core/src/error.rs`, extend `ErrorBody` and `ApiError`:

```rust
#[derive(Debug, serde::Serialize)]
pub struct ErrorBody {
    pub error: ErrorDetail,
    /// Optional payload attached to non-2xx responses that need to convey
    /// state alongside the error (e.g. the current remote row on 409).
    /// Skipped when `None` so existing handlers are unaffected.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub data: Option<serde_json::Value>,
}

impl ApiError {
    /// Build an error response that also carries `data` (any `Serialize`).
    /// The JSON body becomes `{ error: {...}, data: <value> }`.
    pub fn with_data<T: serde::Serialize>(
        status: axum::http::StatusCode,
        code: &str,
        message: &str,
        data: &T,
    ) -> Self {
        let value = match serde_json::to_value(data) {
            Ok(v) => Some(v),
            Err(_) => None,
        };
        ApiError {
            status,
            body: ErrorBody {
                error: ErrorDetail {
                    code: code.to_string(),
                    message: message.to_string(),
                    field: None,
                },
                data: value,
            },
        }
    }
}
```

(Existing `ApiError::new` and `ApiError::validation` keep `data: None` via `Default`; no other handler changes.)

Verify the workspace builds:

```bash
cargo build -p oxipage-core
```

Expected: success.

- [ ] **Step 1: Add `expected_updated_at` to `ProfileInput`**

In `crates/oxipage-ext-profile/src/model.rs`, add the field to `ProfileInput`:

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct ProfileInput {
    /// Last-known `updated_at` from GET /profile. Used for optimistic concurrency.
    /// Use an empty string for unconditional first-write (no prior row).
    pub expected_updated_at: String,
    pub display_name: String,
    pub tagline_ko: Option<String>,
    pub tagline_en: Option<String>,
    pub avatar_url: Option<String>,
    pub bio_ko: Option<String>,
    pub bio_en: Option<String>,
    pub email: Option<String>,
    pub github_username: Option<String>,
    pub linkedin_url: Option<String>,
    #[serde(default)]
    pub education: Vec<Education>,
    #[serde(default)]
    pub custom_links: Vec<CustomLink>,
}
```

- [ ] **Step 2: Add `UpsertError` and `upsert_if_unchanged`**

In `crates/oxipage-ext-profile/src/repo.rs`, append:

```rust
#[derive(Debug, thiserror::Error)]
pub enum UpsertError {
    #[error("stale profile: row changed since {expected}")]
    Stale { expected: String },
    #[error(transparent)]
    Db(#[from] anyhow::Error),
}

pub async fn upsert_if_unchanged(
    pool: &SqlitePool,
    input: &ProfileInput,
) -> Result<Profile, UpsertError> {
    let mut tx = pool.begin().await?;
    let current: Option<(String,)> = sqlx::query_as("SELECT updated_at FROM profile WHERE id = 1")
        .fetch_optional(&mut *tx)
        .await?;
    let current_updated_at = current.as_ref().map(|(s,)| s.clone()).unwrap_or_default();
    if !input.expected_updated_at.is_empty() && current_updated_at != input.expected_updated_at {
        return Err(UpsertError::Stale { expected: input.expected_updated_at.clone() });
    }
    let education = serde_json::to_string(&input.education)?;
    let custom_links = serde_json::to_string(&input.custom_links)?;
    let profile = sqlx::query_as::<_, Profile>(&format!(
        "INSERT INTO profile (id, display_name, tagline_ko, tagline_en, avatar_url, bio_ko, bio_en,
                              email, github_username, linkedin_url, education, custom_links)
         VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
         ON CONFLICT (id) DO UPDATE SET
            display_name = ?1, tagline_ko = ?2, tagline_en = ?3, avatar_url = ?4,
            bio_ko = ?5, bio_en = ?6, email = ?7, github_username = ?8, linkedin_url = ?9,
            education = ?10, custom_links = ?11,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         RETURNING {COLUMNS}"
    ))
    .bind(&input.display_name)
    .bind(&input.tagline_ko)
    .bind(&input.tagline_en)
    .bind(&input.avatar_url)
    .bind(&input.bio_ko)
    .bind(&input.bio_en)
    .bind(&input.email)
    .bind(&input.github_username)
    .bind(&input.linkedin_url)
    .bind(education)
    .bind(custom_links)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(profile)
}
```

- [ ] **Step 3: Add `validate_email` and `validate_year_range`**

In `crates/oxipage-ext-profile/src/model.rs` (or a new `validate.rs` sibling), add:

```rust
pub fn validate_email(s: &str) -> bool {
    let mut parts = s.split('@');
    let local = parts.next().unwrap_or("");
    let domain = parts.next().unwrap_or("");
    let mut dparts = domain.split('.');
    let d0 = dparts.next().unwrap_or("");
    let d1 = dparts.next().unwrap_or("");
    !local.is_empty() && !d0.is_empty() && !d1.is_empty() && !s.contains(' ')
}

pub fn validate_year_range(start: Option<i32>, end: Option<i32>) -> bool {
    match (start, end) {
        (Some(s), Some(e)) => s <= e,
        _ => true,
    }
}
```

- [ ] **Step 4: Update `put_profile` route**

In `crates/oxipage-ext-profile/src/routes.rs`, replace `put_profile`:

```rust
pub async fn put_profile(
    Extension(pool): Extension<SiteScopedDb>,
    Json(input): Json<ProfileInput>,
) -> Result<Json<DataEnvelope<Profile>>, ApiError> {
    if input.display_name.trim().is_empty() {
        return Err(ApiError::validation("display_name", "display_name must not be empty"));
    }
    if let Some(email) = &input.email
        && !email.is_empty()
        && !crate::model::validate_email(email)
    {
        return Err(ApiError::validation("email", "email is not a valid address"));
    }
    for e in &input.education {
        if !crate::model::validate_year_range(e.start_year, e.end_year) {
            return Err(ApiError::validation("education", "education end_year must be >= start_year"));
        }
    }
    // On stale detection, read the current remote row so the client can
    // offer Reload/Compare instead of overwriting silently. The 409 body
    // carries `{ error: {...}, data: <current Profile> }`.
    let profile = repo::upsert_if_unchanged(&pool.db, &input)
        .await
        .map_err(|e| match e {
            repo::UpsertError::Stale { expected: _ } => {
                let remote = match repo::get(&pool.db) {
                    Ok(Some(p)) => p,
                    _ => return ApiError::internal(anyhow::anyhow!("profile vanished during stale write")),
                };
                ApiError::with_data(
                    axum::http::StatusCode::CONFLICT,
                    "stale_profile",
                    "profile changed since your last load; reload to see remote changes",
                    &remote,
                )
            }
            repo::UpsertError::Db(err) => ApiError::internal(err),
        })?;
    Ok(Json(DataEnvelope { data: profile }))
 }
```

- [ ] **Step 5: Build**

Run: `cargo build -p oxipage-ext-profile`
Expected: success.

- [ ] **Step 6: Write integration test**

Create `crates/oxipage-ext-profile/tests/stale_put.rs`:

```rust
use oxipage_ext_profile::model::ProfileInput;
use oxipage_ext_profile::repo;

#[tokio::test]
async fn stale_put_returns_409() {
    // Build a SiteScopedDb in a temp DB; reuse the same harness used by other
    // ext-profile tests for AppState/SiteScopedDb construction.
    let pool = test_pool().await;
    seed_profile(&pool, "Alice").await;

    // First successful upsert.
    let v1 = repo::get(&pool).await.unwrap().unwrap().updated_at;
    let input = ProfileInput {
        expected_updated_at: v1.clone(),
        display_name: "Alice v1".into(),
        tagline_ko: None, tagline_en: None,
        avatar_url: None, bio_ko: None, bio_en: None,
        email: None, github_username: None, linkedin_url: None,
        education: vec![], custom_links: vec![],
    };
    let p1 = repo::upsert_if_unchanged(&pool, &input).await.expect("first write ok");
    let _ = p1;

    // Second PUT reusing the OLD expected_updated_at → must report Stale.
    let stale_input = ProfileInput {
        expected_updated_at: v1,
        display_name: "Alice v2".into(),
        tagline_ko: None, tagline_en: None,
        avatar_url: None, bio_ko: None, bio_en: None,
        email: None, github_username: None, linkedin_url: None,
        education: vec![], custom_links: vec![],
    };
    let err = repo::upsert_if_unchanged(&pool, &stale_input).await.unwrap_err();
    assert!(matches!(err, repo::UpsertError::Stale { .. }), "got {err:?}");
}
```

(Helpers `test_pool()` and `seed_profile()` are defined by the engineer in the test file using the same DB bootstrap pattern used by existing ext-profile tests.)

- [ ] **Step 7: Run test**

Run: `cargo test -p oxipage-ext-profile --test stale_put`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/oxipage-ext-profile/
git commit -m "feat(profile): optimistic-concurrency PUT with expected_updated_at"
```

---

### Task 12: Profile tab in `ContentPage` + new `ProfileTab.tsx`

**Files:**
- Modify: `web/src/admin/content/ContentPage.tsx`
- Create: `web/src/admin/content/ProfileTab.tsx`

**Interfaces:**
- Produces: `ProfileTab({ slug }: { slug: string })` — 14-field form with `education`/`custom_links` repeaters, GET-before-enable-Save, 409 reload surface.

- [ ] **Step 1: Add `profile` tab to `ContentPage`**

In `web/src/admin/content/ContentPage.tsx`, replace the `tabs` and `tabComponents` arrays:

```tsx
import { ProfileTab } from "./ProfileTab";

const tabs = [
  { id: "profile", label: "Profile" },
  { id: "blog", label: "Blog" },
  { id: "projects", label: "Projects" },
  { id: "links", label: "Links" },
  { id: "movies", label: "Movies" },
  { id: "books", label: "Books" },
  { id: "novels", label: "Novels" },
  { id: "scraps", label: "Scraps" },
] as const;

const tabComponents: Record<string, React.FC<{ slug: string }>> = {
  profile: ProfileTab,
  blog: BlogTab,
  projects: ProjectsTab,
  links: LinksTab,
  movies: MoviesTab,
  books: BooksTab,
  novels: NovelsTab,
  scraps: ScrapsTab,
};
```

Also change the initial active tab to `"profile"`:

```tsx
const [activeTab, setActiveTab] = useState("profile");
```

- [ ] **Step 2: Create `ProfileTab.tsx`**

Create `web/src/admin/content/ProfileTab.tsx`:

```tsx
import { useState, useEffect } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Plus, Trash2 } from "lucide-react";

import { Button } from "../../shared/ui/button";
import { DrawerField } from "../../shared/ui/drawer";
import { Input } from "../../shared/ui/input";
import { MarkdownEditor } from "../shared/ui/MarkdownEditor";
import { ImageField } from "../shared/ui/ImageField";
import {
  jsonOrThrow, siteScopedFetch, ApiValidationError, type ProfileData,
} from "../../shared/api";
import { validateEmail, validateDateRange } from "../shared/validation";
import { EditorPreviewDrawer } from "../shared/ui/EditorPreviewDrawer";
import { DraftPreviewPane } from "../shared/ui/DraftPreviewPane";
import { ProfileView as ProfileViewCmp } from "../../extensions/profile/ProfileView";

interface EducationRow {
  institution: string;
  degree: string;
  field: string;
  start_year: string;
  end_year: string;
}

interface CustomLinkRow {
  label: string;
  url: string;
  icon: string;
}

interface FormState {
  display_name: string;
  tagline_ko: string;
  tagline_en: string;
  avatar_url: string;
  bio_ko: string;
  bio_en: string;
  email: string;
  github_username: string;
  linkedin_url: string;
  education: EducationRow[];
  custom_links: CustomLinkRow[];
}

const EMPTY: FormState = {
  display_name: "",
  tagline_ko: "",
  tagline_en: "",
  avatar_url: "",
  bio_ko: "",
  bio_en: "",
  email: "",
  github_username: "",
  linkedin_url: "",
  education: [],
  custom_links: [],
};

export function ProfileTab({ slug }: { slug: string }) {
  const qc = useQueryClient();
  const [open, setOpen] = useState(false);
  const [form, setForm] = useState<FormState>(EMPTY);
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [updatedAt, setUpdatedAt] = useState("");
  const [staleNotice, setStaleNotice] = useState<{ local: FormState; remote: ProfileData } | null>(null);
  const [dirty, setDirty] = useState(false);

  const load = async () => {
    const res = await siteScopedFetch(slug, "/profile");
    if (res.status === 404) {
      setUpdatedAt("");
      setForm(EMPTY);
      setDirty(false);
      return;
    }
    const body = await jsonOrThrow<{ data: ProfileData }>(res);
    setUpdatedAt(body.data.updated_at);
    setForm({
      display_name: body.data.display_name,
      tagline_ko: body.data.tagline_ko ?? "",
      tagline_en: body.data.tagline_en ?? "",
      avatar_url: body.data.avatar_url ?? "",
      bio_ko: body.data.bio_ko ?? "",
      bio_en: body.data.bio_en ?? "",
      email: body.data.email ?? "",
      github_username: body.data.github_username ?? "",
      linkedin_url: body.data.linkedin_url ?? "",
      education: body.data.education.map((e) => ({
        institution: e.institution ?? "",
        degree: e.degree ?? "",
        field: e.field ?? "",
        start_year: e.start_year != null ? String(e.start_year) : "",
        end_year: e.end_year != null ? String(e.end_year) : "",
      })),
      custom_links: body.data.custom_links.map((l) => ({
        label: l.label,
        url: l.url,
        icon: l.icon ?? "",
      })),
    });
    setDirty(false);
  };

  useEffect(() => { load(); /* eslint-disable-next-line */ }, []);

  const validate = (): boolean => {
    const e: Record<string, string> = {};
    if (!form.display_name.trim()) e.display_name = "Display name is required";
    const emailErr = validateEmail(form.email);
    if (emailErr) e.email = emailErr;
    for (let i = 0; i < form.education.length; i++) {
      const r = form.education[i];
      const dr = validateDateRange(r.start_year, r.end_year);
      if (dr) e[`education.${i}`] = dr;
    }
    setErrors(e);
    return Object.keys(e).length === 0;
  };

  const save = useMutation({
    mutationFn: async () => {
      if (!validate()) throw new Error("Please fix the highlighted fields.");
      const res = await siteScopedFetch(slug, "/profile", {
        method: "PUT",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          expected_updated_at: updatedAt,
          display_name: form.display_name.trim(),
          tagline_ko: form.tagline_ko || null,
          tagline_en: form.tagline_en || null,
          avatar_url: form.avatar_url || null,
          bio_ko: form.bio_ko || null,
          bio_en: form.bio_en || null,
          email: form.email || null,
          github_username: form.github_username || null,
          linkedin_url: form.linkedin_url || null,
          education: form.education.map((r) => ({
            institution: r.institution || null,
            degree: r.degree || null,
            field: r.field || null,
            start_year: r.start_year ? Number(r.start_year) : null,
            end_year: r.end_year ? Number(r.end_year) : null,
          })),
          custom_links: form.custom_links.map((l) => ({
            label: l.label,
            url: l.url,
            icon: l.icon || null,
          })),
        }),
      });
      if (res.status === 409) {
        // `jsonOrThrow` throws on non-ok responses, so we cannot use it here —
        // we need the 409 body's `data` (current remote Profile) to offer
        // Reload/Compare. The server emits
        //   { error: { code, message, field }, data: ProfileData }
        // from `ApiError::with_data` on the Stale branch.
        const body = (await res.json().catch(() => null)) as
          | { error?: { code?: string; message?: string }; data?: ProfileData }
          | null;
        const e = new Error(body?.error?.message ?? "stale_profile") as Error & { remote?: ProfileData };
        if (body?.data) e.remote = body.data;
        throw e;
      }
      return jsonOrThrow<{ data: ProfileData }>(res);
    },
    onSuccess: ({ data }) => {
      qc.invalidateQueries({ queryKey: ["profile"] });
      setUpdatedAt(data.updated_at);
      setDirty(false);
      setOpen(false);
    },
    onError: (e: unknown) => {
      const err = e as { remote?: ProfileData; message: string };
      if (err.remote) {
        setStaleNotice({ local: form, remote: err.remote });
      } else if (e instanceof ApiValidationError) {
        setErrors({ [e.field]: e.message });
      } else {
        setErrors({ _form: err.message ?? "Save failed" });
      }
    },
  });

  return (
    <div>
      <div className="flex items-center justify-between mb-3">
        <p className="text-sm text-muted">{updatedAt ? `Updated ${updatedAt}` : "Not initialized yet"}</p>
        <Button size="sm" disabled={!updatedAt} onClick={() => setOpen(true)}>
          <Plus size={14} className="mr-1" /> Edit Profile
        </Button>
      </div>

      <EditorPreviewDrawer
        open={open}
        onClose={() => setOpen(false)}
        title="Profile"
        description="Singleton editor — full-replace PUT"
        dirty={dirty}
        editor={
          <div>
            <DrawerField label="Display name" required error={errors.display_name}>
              <Input value={form.display_name} onChange={(e) => { setDirty(true); setForm((f) => ({ ...f, display_name: e.target.value })); }} />
            </DrawerField>
            <div className="grid grid-cols-2 gap-3">
              <DrawerField label="Tagline (Korean)">
                <Input value={form.tagline_ko} onChange={(e) => { setDirty(true); setForm((f) => ({ ...f, tagline_ko: e.target.value })); }} />
              </DrawerField>
              <DrawerField label="Tagline (English)">
                <Input value={form.tagline_en} onChange={(e) => { setDirty(true); setForm((f) => ({ ...f, tagline_en: e.target.value })); }} />
              </DrawerField>
            </div>
            <DrawerField label="Avatar">
              <ImageField value={form.avatar_url} onChange={(v) => { setDirty(true); setForm((f) => ({ ...f, avatar_url: v })); }} extension="profile" />
            </DrawerField>
            <DrawerField label="Email" error={errors.email}>
            <DrawerField label="Email" error={errors.email}>
              <Input value={form.email} onChange={(e) => { setDirty(true); setForm((f) => ({ ...f, email: e.target.value })); }} placeholder="hello@example.com" />
            </DrawerField>
            <div className="grid grid-cols-2 gap-3">
              <DrawerField label="GitHub username">
                <Input value={form.github_username} onChange={(e) => { setDirty(true); setForm((f) => ({ ...f, github_username: e.target.value })); }} />
              </DrawerField>
              <DrawerField label="LinkedIn URL">
                <Input value={form.linkedin_url} onChange={(e) => { setDirty(true); setForm((f) => ({ ...f, linkedin_url: e.target.value })); }} placeholder="https://..." />
              </DrawerField>
            </div>
            <DrawerField label="Bio (Korean)">
              <MarkdownEditor value={form.bio_ko} onChange={(v) => { setDirty(true); setForm((f) => ({ ...f, bio_ko: v })); }} />
            </DrawerField>
            <DrawerField label="Bio (English)">
              <MarkdownEditor value={form.bio_en} onChange={(v) => { setDirty(true); setForm((f) => ({ ...f, bio_en: v })); }} />
            </DrawerField>

            {/* Education repeater */}
            <div className="border-t border-line pt-4 mt-4">
              <div className="flex items-center justify-between mb-2">
                <h3 className="text-sm font-semibold">Education</h3>
                <Button variant="outline" size="sm" onClick={() => { setDirty(true); setForm((f) => ({ ...f, education: [...f.education, { institution: "", degree: "", field: "", start_year: "", end_year: "" }] })); }}>
                  <Plus size={12} /> Add
                </Button>
              </div>
              {form.education.map((row, i) => (
                <div key={i} className="space-y-2 mb-3 border border-line rounded p-3">
                  <div className="grid grid-cols-2 gap-2">
                    <Input placeholder="Institution" value={row.institution} onChange={(e) => { setDirty(true); setForm((f) => ({ ...f, education: f.education.map((r, j) => j === i ? { ...r, institution: e.target.value } : r) })); }} />
                    <Input placeholder="Degree" value={row.degree} onChange={(e) => { setDirty(true); setForm((f) => ({ ...f, education: f.education.map((r, j) => j === i ? { ...r, degree: e.target.value } : r) })); }} />
                  </div>
                  <div className="grid grid-cols-3 gap-2">
                    <Input placeholder="Field" value={row.field} onChange={(e) => { setDirty(true); setForm((f) => ({ ...f, education: f.education.map((r, j) => j === i ? { ...r, field: e.target.value } : r) })); }} />
                    <Input placeholder="Start year" value={row.start_year} onChange={(e) => { setDirty(true); setForm((f) => ({ ...f, education: f.education.map((r, j) => j === i ? { ...r, start_year: e.target.value } : r) })); }} />
                    <Input placeholder="End year" value={row.end_year} onChange={(e) => { setDirty(true); setForm((f) => ({ ...f, education: f.education.map((r, j) => j === i ? { ...r, end_year: e.target.value } : r) })); }} error={errors[`education.${i}`]} />
                  </div>
                  <div className="flex justify-end">
                    <Button variant="ghost" size="sm" onClick={() => { setDirty(true); setForm((f) => ({ ...f, education: f.education.filter((_, j) => j !== i) })); }}>
                      <Trash2 size={12} /> Remove
                    </Button>
                  </div>
                </div>
              ))}
            </div>

            {/* Custom links repeater */}
            <div className="border-t border-line pt-4 mt-4">
              <div className="flex items-center justify-between mb-2">
                <h3 className="text-sm font-semibold">Custom links</h3>
                <Button variant="outline" size="sm" onClick={() => { setDirty(true); setForm((f) => ({ ...f, custom_links: [...f.custom_links, { label: "", url: "", icon: "" }] })); }}>
                  <Plus size={12} /> Add
                </Button>
              </div>
              {form.custom_links.map((row, i) => (
                <div key={i} className="grid grid-cols-3 gap-2 mb-2">
                  <Input placeholder="Label" value={row.label} onChange={(e) => { setDirty(true); setForm((f) => ({ ...f, custom_links: f.custom_links.map((r, j) => j === i ? { ...r, label: e.target.value } : r) })); }} />
                  <Input placeholder="https://..." value={row.url} onChange={(e) => { setDirty(true); setForm((f) => ({ ...f, custom_links: f.custom_links.map((r, j) => j === i ? { ...r, url: e.target.value } : r) })); }} />
                  <div className="flex gap-1">
                    <Input placeholder="icon" value={row.icon} onChange={(e) => { setDirty(true); setForm((f) => ({ ...f, custom_links: f.custom_links.map((r, j) => j === i ? { ...r, icon: e.target.value } : r) })); }} />
                    <Button variant="ghost" size="sm" onClick={() => { setDirty(true); setForm((f) => ({ ...f, custom_links: f.custom_links.filter((_, j) => j !== i) })); }}>
                      <Trash2 size={12} />
                    </Button>
                  </div>
                </div>
              ))}
            </div>

            {errors._form && <p className="text-sm text-red-600 mt-3">{errors._form}</p>}
          </div>
        }
        preview={
          <DraftPreviewPane>
            <ProfileViewCmp
              profile={{
                display_name: form.display_name || "Unnamed",
                tagline_ko: form.tagline_ko || null,
                tagline_en: form.tagline_en || null,
                avatar_url: form.avatar_url || null,
                bio_ko: form.bio_ko || null,
                bio_en: form.bio_en || null,
                email: form.email || null,
                github_username: form.github_username || null,
                linkedin_url: form.linkedin_url || null,
                education: form.education.map((r) => ({
                  institution: r.institution || null,
                  degree: r.degree || null,
                  field: r.field || null,
                  start_year: r.start_year ? Number(r.start_year) : null,
                  end_year: r.end_year ? Number(r.end_year) : null,
                })),
                custom_links: form.custom_links.map((r) => ({ label: r.label, url: r.url, icon: r.icon || null })),
                updated_at: updatedAt,
              }}
              language="ko"
            />
          </DraftPreviewPane>
        }
        footer={
          staleNotice ? (
            <>
              <Button variant="outline" onClick={() => {
                setStaleNotice(null);
                load();
              }}>Reload remote</Button>
              <Button variant="outline" onClick={() => setStaleNotice(null)}>Keep mine</Button>
            </>
          ) : (
            <>
              <Button variant="outline" onClick={() => setOpen(false)} disabled={save.isPending}>Cancel</Button>
              <Button onClick={() => save.mutate()} disabled={save.isPending || !dirty}>
                {save.isPending ? "Saving..." : "Save"}
              </Button>
            </>
          )
        }
      />
    </div>
  );
}
```

- [ ] **Step 3: Type-check**

Run: `cd web && npx tsc --noEmit`
Expected: success.

- [ ] **Step 4: Smoke**

Run: `cd web && bun run dev`; navigate to Content → Profile; open the editor; verify Save is disabled until GET completes; modify a field and observe Draft Preview updates; trigger a manual 409 (modify DB directly with `sqlite3 data/<site>.sqlite "UPDATE profile SET display_name='RemoteChange' WHERE id=1"`) then save local — observe Reload/Keep footer.

- [ ] **Step 5: Commit**

```bash
git add web/src/admin/content/ProfileTab.tsx web/src/admin/content/ContentPage.tsx
git commit -m "feat(admin): Profile tab with optimistic-concurrency"
```

---

### Task 13: Atomic chapter reorder (novels)

**Files:**
- Modify: `crates/oxipage-ext-novels/src/model.rs`
- Modify: `crates/oxipage-ext-novels/src/repo.rs`
- Modify: `crates/oxipage-ext-novels/src/routes.rs`
- Modify: `crates/oxipage-ext-novels/src/lib.rs`

**Interfaces:**
- Produces:
  ```rust
  pub struct ChapterOrderInput { pub chapter_ids: Vec<i64> }
  pub async fn reorder_chapters(pool, novel_slug, ids) -> Result<Vec<NovelChapter>>; // RepoError::StaleOrder if mismatch
  ```

- [ ] **Step 1: Add `ChapterOrderInput`**

In `crates/oxipage-ext-novels/src/model.rs`, append:

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct ChapterOrderInput {
    pub chapter_ids: Vec<i64>,
}
```

- [ ] **Step 2: Add `reorder_chapters`**

In `crates/oxipage-ext-novels/src/repo.rs`, append:

```rust
use std::collections::HashSet;

pub async fn reorder_chapters(
    pool: &sqlx::SqlitePool,
    novel_slug: &str,
    ids: &[i64],
) -> anyhow::Result<Vec<crate::model::NovelChapter>> {
    let novel_id = novel_id(pool, novel_slug).await?;
    let mut tx = pool.begin().await?;
    let current: Vec<(i64,)> = sqlx::query_as("SELECT id FROM chapter WHERE novel_id = ? ORDER BY chapter_order")
        .bind(novel_id)
        .fetch_all(&mut *tx)
        .await?;
    let current_ids: Vec<i64> = current.into_iter().map(|(i,)| i).collect();

    // Exact set equality required: same length, same membership.
    if current_ids.len() != ids.len()
        || current_ids.iter().collect::<HashSet<_>>() != ids.iter().collect::<HashSet<_>>()
    {
        anyhow::bail!("stale_order: submitted IDs do not match current chapter set");
    }

    for (idx, id) in ids.iter().enumerate() {
        sqlx::query("UPDATE chapter SET chapter_order = ?1 WHERE id = ?2 AND novel_id = ?3")
            .bind((idx as i32) + 1)
            .bind(id)
            .bind(novel_id)
            .execute(&mut *tx)
            .await?;
    }
    let updated = sqlx::query_as::<_, crate::model::NovelChapter>(&format!(
        "SELECT {CHAPTER_COLUMNS} FROM chapter WHERE novel_id = ? ORDER BY chapter_order"
    ))
    .bind(novel_id)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(updated)
}
```

- [ ] **Step 3: Add handler `reorder_chapters`**

In `crates/oxipage-ext-novels/src/routes.rs`, append:

```rust
pub async fn reorder_chapters(
    Extension(pool): Extension<SiteScopedDb>,
    Path(slug): Path<String>,
    Json(input): Json<ChapterOrderInput>,
) -> Result<Json<DataEnvelope<Vec<NovelChapter>>>, ApiError> {
    if input.chapter_ids.iter().collect::<std::collections::HashSet<_>>().len() != input.chapter_ids.len() {
        return Err(ApiError::validation("chapter_ids", "chapter_ids contains duplicates"));
    }
    let chapters = repo::reorder_chapters(&pool.db, &slug, &input.chapter_ids)
        .await
        .map_err(|e| {
            let msg = e.to_string();
            if msg.starts_with("stale_order") {
                ApiError::new(axum::http::StatusCode::CONFLICT, "stale_order", "submitted IDs do not match current chapter set")
            } else {
                ApiError::internal(e)
            }
        })?;
    Ok(Json(DataEnvelope { data: chapters }))
}
```

- [ ] **Step 4: Wire route in `lib.rs`**

In `crates/oxipage-ext-novels/src/lib.rs`, inside `routes(&self) -> Router`, add:

```rust
.route(
    "/{slug}/chapters/order",
    axum::routing::put(routes::reorder_chapters),
)
```

- [ ] **Step 5: Server validation for `cover_image`**

In `crates/oxipage-ext-novels/src/routes.rs`, in `create_novel`, append:

```rust
if let Some(ref url) = input.cover_image
    && !url.is_empty()
    && !oxipage_core::validation::is_image_value(url)
{
    return Err(ApiError::validation("cover_image", "cover_image must be an http(s) URL or site media path"));
}
```

(The helper `oxipage_core::validation::is_image_value` is added in Task 14.)

- [ ] **Step 6: Build + test**

Run: `cargo build -p oxipage-ext-novels` then `cargo test -p oxipage-ext-novels`.
Expected: build OK; existing tests still pass.

- [ ] **Step 7: Write integration test for atomic reorder**

Create `crates/oxipage-ext-novels/tests/reorder_chapters.rs`:

```rust
use oxipage_ext_novels::repo;

#[tokio::test]
async fn reorder_rejects_partial_set() {
    let pool = test_pool().await; // reuse the helper used by other ext-novels tests
    seed_three_chapters(&pool, "novel-a").await;
    let err = repo::reorder_chapters(&pool, "novel-a", &[1, 2]).await.unwrap_err();
    assert!(err.to_string().starts_with("stale_order"));
}
```

- [ ] **Step 8: Commit**

```bash
git add crates/oxipage-ext-novels/
git commit -m "feat(novels): atomic chapter reorder + cover_image validation"
```

---

### Task 14: Atomic screenshot reorder (projects) + core validators

**Files:**
- Create: `crates/oxipage-core/src/validation.rs`
- Modify: `crates/oxipage-core/src/lib.rs` (export new module)
- Modify: `crates/oxipage-ext-projects/src/model.rs`
- Modify: `crates/oxipage-ext-projects/src/repo.rs`
- Modify: `crates/oxipage-ext-projects/src/routes.rs`
- Modify: `crates/oxipage-ext-projects/src/lib.rs`

**Interfaces:**
- Produces:
  ```rust
  // crates/oxipage-core/src/validation.rs
  pub fn is_http_url(s: &str) -> bool;
  pub fn is_media_path(s: &str) -> bool;
  pub fn is_image_value(s: &str) -> bool;
  pub fn clamp_rating(v: i8) -> Option<i8>;
  pub fn validate_year(v: i32) -> Option<i32>;
  pub fn validate_email(s: &str) -> bool;
  pub fn validate_isbn13(s: &str) -> bool;
  pub fn validate_date_order(start: Option<&str>, end: Option<&str>) -> bool;
  pub fn validate_year_order(start: Option<i32>, end: Option<i32>) -> bool;
  ```

- [ ] **Step 1: Create `validation.rs` in core**

Create `crates/oxipage-core/src/validation.rs`:

```rust
//! Shared server-side validators. Single source of truth mirrored by
//! `web/src/admin/shared/validation.ts`.

pub fn is_http_url(s: &str) -> bool {
    s.starts_with("http://") || s.starts_with("https://")
}

pub fn is_media_path(s: &str) -> bool {
    if s.starts_with('/') || s.starts_with('.') || s.contains("..") { return false; }
    if s.starts_with("javascript:") || s.starts_with("data:") || s.starts_with("file:") { return false; }
    let mut parts = s.splitn(3, '/');
    let kind = parts.next();
    let ext = parts.next();
    let file = parts.next();
    let valid_kind = matches!(kind, Some("media"));
    let valid_ext = ext.map(|e| !e.is_empty() && e.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')).unwrap_or(false);
    let valid_file = file.map(|f| !f.is_empty() && f.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')).unwrap_or(false);
    valid_kind && valid_ext && valid_file
}

pub fn is_image_value(s: &str) -> bool {
    is_http_url(s) || is_media_path(s)
}

pub fn clamp_rating(v: i8) -> Option<i8> {
    if (0..=10).contains(&v) { Some(v) } else { None }
}

pub fn validate_year(v: i32) -> Option<i32> {
    if (1000..=9999).contains(&v) { Some(v) } else { None }
}

pub fn validate_email(s: &str) -> bool {
    let mut at = s.split('@');
    let local = at.next().unwrap_or("");
    let domain = at.next().unwrap_or("");
    let mut dp = domain.split('.');
    let d0 = dp.next().unwrap_or("");
    let d1 = dp.next().unwrap_or("");
    !local.is_empty() && !d0.is_empty() && !d1.is_empty() && !s.contains(' ')
}

pub fn validate_isbn13(s: &str) -> bool {
    let t: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
    if t.len() != 13 { return false; }
    let mut sum = 0i32;
    for (i, ch) in t.chars().enumerate().take(12) {
        let d = ch.to_digit(10).unwrap() as i32;
        sum += if i % 2 == 0 { d } else { d * 3 };
    }
    let check = (10 - (sum % 10)) % 10;
    check == t.chars().last().unwrap().to_digit(10).unwrap() as i32
}

pub fn validate_date_order(start: Option<&str>, end: Option<&str>) -> bool {
    match (start, end) {
        (Some(s), Some(e)) if !s.is_empty() && !e.is_empty() => s <= e,
        _ => true,
    }
}

pub fn validate_year_order(start: Option<i32>, end: Option<i32>) -> bool {
    match (start, end) {
        (Some(s), Some(e)) => s <= e,
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn urls() { assert!(is_http_url("https://x")); assert!(!is_http_url("ftp://x")); }
    #[test] fn media() { assert!(is_media_path("media/profile/a.webp")); assert!(!is_media_path("/media/x")); assert!(!is_media_path("media/x/../y")); assert!(!is_media_path("javascript:alert(1)")); }
    #[test] fn isbn() { assert!(validate_isbn13("9780306406157")); assert!(!validate_isbn13("9780306406150")); }
    #[test] fn email() { assert!(validate_email("a@b.co")); assert!(!validate_email("a@b")); }
    #[test] fn date() { assert!(validate_date_order(Some("2024-01-01"), Some("2024-02-01"))); assert!(!validate_date_order(Some("2024-02-01"), Some("2024-01-01"))); assert!(validate_date_order(None, Some("2024-01-01"))); }
    #[test] fn year_order() { assert!(validate_year_order(Some(2020), Some(2024))); assert!(!validate_year_order(Some(2024), Some(2020))); }
}
```

- [ ] **Step 2: Export module in `crates/oxipage-core/src/lib.rs`**

Add: `pub mod validation;`

- [ ] **Step 3: Write `ScreenshotOrderInput` in projects model**

In `crates/oxipage-ext-projects/src/model.rs`, append:

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct ScreenshotOrderInput {
    pub screenshot_ids: Vec<i64>,
}
```

- [ ] **Step 4: Add `reorder_screenshots` in projects repo**

In `crates/oxipage-ext-projects/src/repo.rs`, append:

```rust
use std::collections::HashSet;

pub async fn reorder_screenshots(
    pool: &sqlx::SqlitePool,
    project_slug: &str,
    ids: &[i64],
) -> anyhow::Result<Vec<crate::model::Screenshot>> {
    let project_id: i64 = sqlx::query_scalar("SELECT id FROM project WHERE slug = ?")
        .bind(project_slug)
        .fetch_one(pool)
        .await?;
    let mut tx = pool.begin().await?;
    let current: Vec<(i64,)> = sqlx::query_as("SELECT id FROM project_screenshot WHERE project_id = ? ORDER BY display_order")
        .bind(project_id)
        .fetch_all(&mut *tx)
        .await?;
    let current_ids: Vec<i64> = current.into_iter().map(|(i,)| i).collect();
    if current_ids.len() != ids.len() || current_ids.iter().collect::<HashSet<_>>() != ids.iter().collect::<HashSet<_>>() {
        anyhow::bail!("stale_order: submitted IDs do not match current screenshot set");
    }
    for (idx, id) in ids.iter().enumerate() {
        sqlx::query("UPDATE project_screenshot SET display_order = ?1 WHERE id = ?2 AND project_id = ?3")
            .bind(idx as i32)
            .bind(id)
            .bind(project_id)
            .execute(&mut *tx)
            .await?;
    }
    let updated = sqlx::query_as::<_, crate::model::Screenshot>(&format!(
        "SELECT id, project_id, url, alt_ko, alt_en, display_order, created_at FROM project_screenshot WHERE project_id = ? ORDER BY display_order"
    ))
    .bind(project_id)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(updated)
}
```

- [ ] **Step 5: Add route handler in projects routes**

In `crates/oxipage-ext-projects/src/routes.rs`, append:

```rust
pub async fn reorder_screenshots(
    Extension(pool): Extension<SiteScopedDb>,
    Path(slug): Path<String>,
    Json(input): Json<crate::model::ScreenshotOrderInput>,
) -> Result<Json<DataEnvelope<Vec<crate::model::Screenshot>>>, ApiError> {
    use std::collections::HashSet;
    if input.screenshot_ids.iter().collect::<HashSet<_>>().len() != input.screenshot_ids.len() {
        return Err(ApiError::validation("screenshot_ids", "screenshot_ids contains duplicates"));
    }
    let shots = repo::reorder_screenshots(&pool.db, &slug, &input.screenshot_ids)
        .await
        .map_err(|e| {
            let msg = e.to_string();
            if msg.starts_with("stale_order") {
                ApiError::new(axum::http::StatusCode::CONFLICT, "stale_order", "submitted IDs do not match current screenshot set")
            } else {
                ApiError::internal(e)
            }
        })?;
    Ok(Json(DataEnvelope { data: shots }))
}
```

- [ ] **Step 6: Wire route in projects lib.rs**

In `crates/oxipage-ext-projects/src/lib.rs`, add:

```rust
.route("/{slug}/screenshots/order", axum::routing::put(routes::reorder_screenshots))
```

- [ ] **Step 7: Validate screenshot URL and date order in projects routes**

In `crates/oxipage-ext-projects/src/routes.rs`, in `add_screenshot`, after the existing checks, add:

```rust
if !oxipage_core::validation::is_image_value(&input.url) {
    return Err(ApiError::validation("url", "url must be an http(s) URL or site media path"));
}
```

Also extend `validate_input` to use the shared date-order helper (replacing the existing inline check):

```rust
if !oxipage_core::validation::validate_date_order(
    input.started_at.as_deref().filter(|s| !s.is_empty()),
    input.ended_at.as_deref().filter(|s| !s.is_empty()),
) {
    return Err(ApiError::validation("ended_at", "ended_at must not precede started_at"));
}
```

- [ ] **Step 8: Build + test**

Run: `cargo build -p oxipage-core -p oxipage-ext-projects` then `cargo test -p oxipage-core --lib validation && cargo test -p oxipage-ext-projects`.
Expected: success.

- [ ] **Step 9: Commit**

```bash
git add crates/oxipage-core/src/validation.rs crates/oxipage-core/src/lib.rs crates/oxipage-ext-projects/
git commit -m "feat(projects): atomic screenshot reorder + core validators"
```

---

### Task 15: Blog tab improvements (TagInput, language from config, draft preview)

**Files:**
- Modify: `web/src/admin/content/BlogTab.tsx`

**Interfaces:**
- Produces: blog form uses `TagInput` for `tags`, language `<select>` populated from site config `languages`, drawer replaced by `EditorPreviewDrawer` rendering `DraftPreviewPane → BlogPostView`.

- [ ] **Step 1: Convert `BlogTab` to use `EditorPreviewDrawer`, `TagInput`, site languages**

In `web/src/admin/content/BlogTab.tsx`, replace the form-state, drawer, and language select:

```tsx
import { useQuery } from "@tanstack/react-query";
import { ApiValidationError, contentClient, getConfig } from "../shared/api";
import { TagInput } from "../shared/ui/TagInput";
import { EditorPreviewDrawer } from "../shared/ui/EditorPreviewDrawer";
import { DraftPreviewPane } from "../shared/ui/DraftPreviewPane";
import { BlogPostView } from "../../extensions/blog/BlogPostView";

// Form shape with string[] tags.
interface FormState { title: string; body: string; lang: string; tags: string[]; }
const EMPTY: FormState = { title: "", body: "", lang: "ko", tags: [] };

// inside the component:
const { data: cfg } = useQuery({ queryKey: ["site", slug, "config"], queryFn: () => getConfig(slug) });
const enabledLangs = (cfg?.site?.languages ?? []).map((l) => l.code);

// <select> becomes:
<select
  value={form.lang}
  onChange={(e) => setForm((f) => ({ ...f, lang: e.target.value }))}
  className="h-10 w-full rounded-md border border-line bg-canvas px-3 text-sm text-foreground"
>
  {enabledLangs.map((c) => <option key={c} value={c}>{c}</option>)}
</select>

// Tags field replaced with:
<DrawerField label="Tags">
  <TagInput value={form.tags} onChange={(tags) => setForm((f) => ({ ...f, tags }))} />
</DrawerField>

// save mutation sends `tags: form.tags` (no split). Validation: trim title required.
```

Replace the existing `<Drawer>` with `<EditorPreviewDrawer>`:

```tsx
<EditorPreviewDrawer
  open={editing !== null}
  onClose={() => setEditing(null)}
  title={editing === "new" ? "New Post" : "Edit Post"}
  description={editing !== null && editing !== "new" ? `/${editing.slug}` : "Create a new blog post draft"}
  dirty={JSON.stringify(form) !== JSON.stringify(initialForm)}
  editor={
    <div>
      <DrawerField label="Title" required error={errors.title}>
        <Input value={form.title} onChange={(e) => setForm((f) => ({ ...f, title: e.target.value }))} autoFocus />
      </DrawerField>
      <DrawerField label="Language">
        <select value={form.lang} onChange={(e) => setForm((f) => ({ ...f, lang: e.target.value }))} className="h-10 w-full rounded-md border border-line bg-canvas px-3 text-sm text-foreground">
          {enabledLangs.map((c) => <option key={c} value={c}>{c}</option>)}
        </select>
      </DrawerField>
      <DrawerField label="Tags"><TagInput value={form.tags} onChange={(tags) => setForm((f) => ({ ...f, tags }))} /></DrawerField>
      <DrawerField label="Body"><MarkdownEditor value={form.body} onChange={(v) => setForm((f) => ({ ...f, body: v }))} rows={16} /></DrawerField>
      {errors._form && <p className="text-sm text-red-600">{errors._form}</p>}
    </div>
  }
  preview={
    <DraftPreviewPane>
      <BlogPostView post={{ title: form.title || "Untitled", body: form.body, lang: form.lang as "ko" | "en", tags: form.tags, published_at: null, created_at: new Date().toISOString() }} language="ko" />
    </DraftPreviewPane>
  }
  footer={
    <>
      <Button variant="outline" onClick={() => setEditing(null)} disabled={save.isPending}>Cancel</Button>
      <Button onClick={() => save.mutate()} disabled={save.isPending || !form.title.trim()}>
        {save.isPending ? "Saving..." : "Save"}
      </Button>
    </>
  }
/>
```

- [ ] **Step 2: Add per-field error capture from `ApiValidationError`**

In the `save` mutation `onError`:

```tsx
onError: (e) => {
  if (e instanceof ApiValidationError) {
    setErrors({ [e.field]: e.message });
  } else {
    setErrors({ _form: e instanceof Error ? e.message : "Save failed" });
  }
},
```

- [ ] **Step 3: Type-check**

Run: `cd web && npx tsc --noEmit`
Expected: success.

- [ ] **Step 4: Smoke**

Run: `cd web && bun run dev`; navigate to Content → Blog; create a new post; observe TagInput, draft preview updating live.

- [ ] **Step 5: Commit**

```bash
git add web/src/admin/content/BlogTab.tsx
git commit -m "feat(blog): TagInput, language from site config, draft preview"
```

---

### Task 16: Books tab improvements (status enum fix, ImageField cover, MarkdownEditor reviews, ISBN-13)

**Files:**
- Modify: `web/src/admin/content/BooksTab.tsx`
- Modify: `crates/oxipage-ext-books/src/routes.rs`

**Interfaces:**
- Produces:
  - Books form: status enum `wishlist|reading|completed|dropped` only.
  - Books cover uses `ImageField`.
  - Books review fields use `MarkdownEditor`.
  - Server: ISBN-13 checksum + `finished_at >= started_at` validation.

- [ ] **Step 1: Fix status enum and add cover/review markdown in `BooksTab.tsx`**

In `web/src/admin/content/BooksTab.tsx`:

```tsx
// status: change default + options
status: "wishlist",  // was "read"
<option value="wishlist">wishlist</option>
<option value="reading">reading</option>
<option value="completed">completed</option>
<option value="dropped">dropped</option>
// remove read/dnf

// Cover row:
<DrawerField label="Cover">
  <ImageField value={form.cover_image_url ?? ""} onChange={(v) => setForm((f) => ({ ...f, cover_image_url: v }))} extension="books" error={errors.cover_image_url} />
</DrawerField>

// Reviews: replace Textarea with MarkdownEditor
import { MarkdownEditor } from "../shared/ui/MarkdownEditor";
<MarkdownEditor value={form.review_ko} onChange={(v) => setForm((f) => ({ ...f, review_ko: v }))} rows={6} />
<MarkdownEditor value={form.review_en} onChange={(v) => setForm((f) => ({ ...f, review_en: v }))} rows={6} />

// Add cover_image_url to FormState (and to save payload).
// Wire ApiValidationError → per-field errors (same as Task 15 step 2).
```

- [ ] **Step 2: Add ISBN-13 + date validation in `routes.rs`**

In `crates/oxipage-ext-books/src/routes.rs`, in `validate_create` and `validate_patch`:

```rust
if let Some(isbn) = &input.isbn13
    && !isbn.is_empty()
    && !oxipage_core::validation::validate_isbn13(isbn)
{
    return Err(ApiError::validation("isbn13", "isbn13 is not a valid ISBN-13"));
}
if !oxipage_core::validation::validate_date_order(input.started_at.as_deref(), input.finished_at.as_deref()) {
    return Err(ApiError::validation("finished_at", "finished_at must not precede started_at"));
}
```

- [ ] **Step 3: Build + test**

Run: `cargo build -p oxipage-ext-books` then `cargo test -p oxipage-ext-books`.
Expected: success.

- [ ] **Step 4: Smoke**

Open Books tab; create a book; verify dropdown shows only the four valid statuses; bad ISBN-13 surfaces an inline error after save.

- [ ] **Step 5: Commit**

```bash
git add web/src/admin/content/BooksTab.tsx crates/oxipage-ext-books/
git commit -m "feat(books): status enum fix, ImageField cover, ISBN-13 + date validation"
```

---

### Task 17: Projects tab improvements (ImageField screenshots, links repeater, dates, atomic reorder)

**Files:**
- Modify: `web/src/admin/content/ProjectsTab.tsx`
- Modify: `web/src/admin/shared/api.ts` (add `reorderScreenshots`)

**Interfaces:**
- Produces:
  - Project form has `started_at` and `ended_at` `<input type="date">`.
  - Screenshot editor uses `ImageField` and shows atomic reorder UI.
  - Links editor is a `custom_links` repeater (matches Rust model field — already JSON).
  - KO and EN alt text per screenshot.

- [ ] **Step 1: Extend `ProjectsTab` form state and add new fields**

In `web/src/admin/content/ProjectsTab.tsx`, extend the form:

```tsx
interface FormState {
  title_ko: string;
  title_en: string;
  description_ko: string;
  description_en: string;
  tech_stack: string;        // comma-separated for editing
  status: string;
  started_at: string;        // YYYY-MM-DD
  ended_at: string;
  links_custom: { label: string; url: string }[]; // JSON field
  screenshots: { id?: number; url: string; alt_ko: string; alt_en: string; display_order: number; uploading?: boolean }[];
  featured: boolean;
}
```

Add `started_at`, `ended_at` editors in the drawer (date inputs). Add a links repeater (same shape as Profile's custom links).

Add screenshot editor section:

```tsx
<div className="border-t border-line pt-4 mt-4">
  <h3 className="text-sm font-semibold mb-2">Screenshots</h3>
  {form.screenshots.map((s, i) => (
    <div key={i} className="space-y-2 mb-3 border border-line rounded p-3">
      <ImageField value={s.url} onChange={(v) => updateScreenshot(i, { url: v })} extension="projects" error={errors[`screenshots.${i}.url`]} />
      <div className="grid grid-cols-2 gap-2">
        <Input placeholder="Alt (ko)" value={s.alt_ko} onChange={(e) => updateScreenshot(i, { alt_ko: e.target.value })} />
        <Input placeholder="Alt (en)" value={s.alt_en} onChange={(e) => updateScreenshot(i, { alt_en: e.target.value })} />
      </div>
      <Button variant="ghost" size="sm" onClick={() => removeScreenshot(i)}><Trash2 size={12} /> Remove</Button>
    </div>
  ))}
  <Button variant="outline" size="sm" onClick={addScreenshot}><Plus size={12} /> Add</Button>
</div>
```

Add a save-time call to `reorderScreenshots` if the user rearranged: client builds the new `screenshot_ids` order (after each reorder update) and sends a single PUT before project save.

- [ ] **Step 2: Add `reorderScreenshots` to `web/src/admin/shared/api.ts`**

In `web/src/admin/shared/api.ts`, near the existing `addScreenshot`/`updateScreenshot`/`deleteScreenshot`, append:

```ts
export async function reorderScreenshots(
  slug: string,
  projectSlug: string,
  ids: number[],
): Promise<Screenshot[]> {
  const res = await siteScopedFetch(slug, `/projects/${projectSlug}/screenshots/order`, {
    method: "PUT",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ screenshot_ids: ids }),
  });
  return jsonOrThrow<{ data: Screenshot[] }>(res).then((b) => b.data);
}
```

(Confirm `Screenshot` interface already exists in the file.)

- [ ] **Step 3: Type-check and smoke**

Run: `cd web && npx tsc --noEmit`; smoke by creating a project with multiple screenshots; reorder; save.

- [ ] **Step 4: Commit**

```bash
git add web/src/admin/content/ProjectsTab.tsx web/src/admin/shared/api.ts
git commit -m "feat(projects): ImageField screenshots, links repeater, dates, atomic reorder"
```

---

### Task 18: Movies tab improvements (TMDB search, ImageField series cover, remove `any`)

**Files:**
- Modify: `web/src/admin/content/MoviesTab.tsx`
- Modify: `crates/oxipage-ext-movies/src/routes.rs`
- Modify: `web/src/admin/shared/api.ts` (add `searchTmdb`)

**Interfaces:**
- Produces:
  - Movies tab: search box calls `GET /movies/search?q=...`, results table lets user click → open drawer prefilled with `title`, `media_type`, `release_year`, `poster_path`.
  - Series group editor exposes `cover_image` via `ImageField`.
  - Movies form typed (no `any`); type `MovieEntry` and `SeriesGroupDetail` from API.

- [ ] **Step 1: Add `searchTmdb` client**

In `web/src/admin/shared/api.ts`, append:

```ts
export async function searchTmdb(slug: string, q: string): Promise<TmdbSearchResult[]> {
  const res = await siteScopedFetch(slug, `/movies/search?q=${encodeURIComponent(q)}`);
  return jsonOrThrow<{ data: TmdbSearchResult[] }>(res).then((b) => b.data);
}

export interface TmdbSearchResult {
  tmdb_id: number;
  title: string;
  media_type: "movie" | "tv";
  poster_path: string | null;
  release_year: number | null;
}
```

(Define `TmdbSearchResult` to match the Rust server struct.)

- [ ] **Step 2: Wire search in MoviesTab**

In `web/src/admin/content/MoviesTab.tsx`, add a search row above the table:

```tsx
const [q, setQ] = useState("");
const search = useQuery({
  queryKey: ["site", slug, "movies", "search", q],
  queryFn: () => searchTmdb(slug, q),
  enabled: q.trim().length > 1,
});
```

When a result is clicked, open `editing="new"` and prefill the form with the result fields. Remove any `any` types — replace with `MovieEntry`/`SeriesGroupDetail` imports.

- [ ] **Step 3: Series group drawer with `ImageField`**

In the same file, replace the series-group editor's cover input with `ImageField`:

```tsx
import { ImageField } from "../shared/ui/ImageField";
<DrawerField label="Cover image">
  <ImageField value={seriesForm.cover_image ?? ""} onChange={(v) => setSeriesForm((f) => ({ ...f, cover_image: v }))} extension="movies" />
</DrawerField>
```

- [ ] **Step 4: Add release_year and series order validation in routes**

In `crates/oxipage-ext-movies/src/routes.rs`, in `create`:

```rust
if let Some(y) = input.release_year
    && oxipage_core::validation::validate_year(y).is_none()
{
    return Err(ApiError::validation("release_year", "release_year must be a 4-digit year"));
}
if let Some(o) = input.series_order
    && o <= 0
{
    return Err(ApiError::validation("series_order", "series_order must be positive"));
}
```

- [ ] **Step 5: Type-check and smoke**

Run: `cd web && npx tsc --noEmit`; in dev, type a movie title; click a TMDB result; verify drawer prefills fields.

- [ ] **Step 6: Commit**

```bash
git add web/src/admin/content/MoviesTab.tsx web/src/admin/shared/api.ts crates/oxipage-ext-movies/
git commit -m "feat(movies): wire TMDB search, ImageField series cover, typed API"
```

---

### Task 19: Novels tab improvements (ImageField cover, TagInput, atomic chapter reorder)

**Files:**
- Modify: `web/src/admin/content/NovelsTab.tsx`
- Modify: `web/src/admin/shared/api.ts` (add `reorderChapters`)

**Interfaces:**
- Produces: novel form uses `ImageField` for cover and `TagInput` for tags; chapter list uses up/down chevrons that build an atomic reorder PUT (instead of two paired PATCH calls).

- [ ] **Step 1: Add `reorderChapters` client**

In `web/src/admin/shared/api.ts`, append:

```ts
export async function reorderChapters(
  slug: string,
  novelSlug: string,
  ids: number[],
): Promise<NovelChapter[]> {
  const res = await siteScopedFetch(slug, `/novels/${novelSlug}/chapters/order`, {
    method: "PUT",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ chapter_ids: ids }),
  });
  return jsonOrThrow<{ data: NovelChapter[] }>(res).then((b) => b.data);
}
```

- [ ] **Step 2: Replace cover/tags and reorder in `NovelsTab.tsx`**

In `web/src/admin/content/NovelsTab.tsx`:

```tsx
import { ImageField } from "../shared/ui/ImageField";
import { TagInput } from "../shared/ui/TagInput";
import { reorderChapters } from "../shared/api";

// cover_image row becomes:
<DrawerField label="Cover image">
  <ImageField value={form.cover_image} onChange={(v) => setForm((f) => ({ ...f, cover_image: v }))} extension="novels" error={errors.cover_image} />
</DrawerField>

// tags row becomes:
<DrawerField label="Tags">
  <TagInput value={form.tagsArray} onChange={(tags) => setForm((f) => ({ ...f, tagsArray: tags }))} />
</DrawerField>

// replace handleReorder with:
const handleReorder = (order: number, direction: -1 | 1) => {
  const idx = chapters.findIndex((c) => c.chapter_order === order);
  if (idx === -1) return;
  const target = chapters[idx + direction];
  if (!target) return;
  const ids = chapters.map((c) => c.id);
  const tmp = ids[idx];
  ids[idx] = ids[idx + direction];
  ids[idx + direction] = tmp;
  reorderChaptersMut.mutate(ids);
};

const reorderChaptersMut = useMutation({
  mutationFn: (ids: number[]) => reorderChapters(slug!, novelSlug!, ids),
  onSuccess: () => qc.invalidateQueries({ queryKey: ["site", slug, "novels", novelSlug, "chapters"] }),
});

// save mutation sends tags: form.tagsArray (string[]).
```

Add `tagsArray: string[]` to `FormState`. Update `openEdit` and `EMPTY` accordingly. Disable chapter add/save when `chapterForm.title.trim() === ""`.

- [ ] **Step 3: Type-check and smoke**

Run: `cd web && npx tsc --noEmit`; open a novel with ≥ 2 chapters; click up-chevron once; verify only ONE server call (the PUT) and that the chapter list reorders.

- [ ] **Step 4: Commit**

```bash
git add web/src/admin/content/NovelsTab.tsx web/src/admin/shared/api.ts
git commit -m "feat(novels): ImageField cover, TagInput, atomic chapter reorder"
```

---

### Task 20: Links tab improvements (ImageField thumbnail, TagInput)

**Files:**
- Modify: `web/src/admin/content/LinksTab.tsx`
- Modify: `crates/oxipage-ext-links/src/routes.rs`

**Interfaces:**
- Produces: links form uses `ImageField` for thumbnail and `TagInput` for tags.

- [ ] **Step 1: Replace thumbnail and tags in `LinksTab.tsx`**

In `web/src/admin/content/LinksTab.tsx`:

```tsx
import { ImageField } from "../shared/ui/ImageField";
import { TagInput } from "../shared/ui/TagInput";

// Extend FormState:
interface FormState { title: string; url: string; description_ko: string; description_en: string; thumbnail: string; tags: string[]; display_order: string; featured: boolean; }

// Thumbnail row:
<DrawerField label="Thumbnail">
  <ImageField value={form.thumbnail} onChange={(v) => setForm((f) => ({ ...f, thumbnail: v }))} extension="links" error={errors.thumbnail} />
</DrawerField>

// Tags row:
<DrawerField label="Tags">
  <TagInput value={form.tags} onChange={(tags) => setForm((f) => ({ ...f, tags }))} />
</DrawerField>

// Save payload uses `tags: form.tags` (string[]), `thumbnail_url: form.thumbnail || null`, `display_order: Number(form.display_order || 0)`.
```

- [ ] **Step 2: Add server validation in `routes.rs`**

In `crates/oxipage-ext-links/src/routes.rs`, in the create/update handler, add:

```rust
if !oxipage_core::validation::is_http_url(&input.url) {
    return Err(ApiError::validation("url", "url must be an http(s) URL"));
}
if let Some(t) = &input.thumbnail_url
    && !t.is_empty()
    && !oxipage_core::validation::is_image_value(t)
{
    return Err(ApiError::validation("thumbnail_url", "thumbnail_url must be an http(s) URL or site media path"));
}
```

- [ ] **Step 3: Type-check + build**

Run: `cd web && npx tsc --noEmit`; `cargo build -p oxipage-ext-links`.
Expected: success.

- [ ] **Step 4: Smoke + commit**

Smoke: open Links; create with an uploaded thumbnail + several tags. Then:

```bash
git add web/src/admin/content/LinksTab.tsx crates/oxipage-ext-links/
git commit -m "feat(links): ImageField thumbnail, TagInput, http(s) + media path validation"
```

---

### Task 21: Scraps tab improvements (ImageField og override, TagInput, read-only/editable split)

**Files:**
- Modify: `web/src/admin/content/ScrapsTab.tsx`
- Modify: `crates/oxipage-ext-scraps/src/routes.rs`

**Interfaces:**
- Produces: scrap drawer shows read-only `source`, `source_item_id`, `scraped_at`, `source_url`, `title`, original `og_image_url`; editable: `notes_ko/en`, `tags`, image override.

- [ ] **Step 1: Split read-only vs editable in `ScrapsTab.tsx`**

In `web/src/admin/content/ScrapsTab.tsx`, replace the drawer body:

```tsx
<DrawerField label="Source"><Input value={s.source} disabled /></DrawerField>
<DrawerField label="Title"><Input value={s.title} disabled /></DrawerField>
<DrawerField label="Original source URL"><Input value={s.source_url} disabled /></DrawerField>
{s.og_image_url && (
  <div className="mb-4 text-xs text-muted">
    <span>Original OG image: </span>
    <img src={s.og_image_url} alt="" className="inline-block size-12 rounded-md border border-line object-cover" />
  </div>
)}
<DrawerField label="Image override">
  <ImageField value={form.image_override} onChange={(v) => setForm((f) => ({ ...f, image_override: v }))} extension="scraps" error={errors.og_image_url} />
</DrawerField>
<DrawerField label="Note (Korean)">
  <MarkdownEditor value={form.note_ko} onChange={(v) => setForm((f) => ({ ...f, note_ko: v }))} />
</DrawerField>
<DrawerField label="Note (English)">
  <MarkdownEditor value={form.note_en} onChange={(v) => setForm((f) => ({ ...f, note_en: v }))} />
</DrawerField>
<DrawerField label="Tags">
  <TagInput value={form.tags} onChange={(tags) => setForm((f) => ({ ...f, tags }))} />
</DrawerField>

// Save payload uses `tags: form.tags`, `note_ko/en`, `og_image_url: form.image_override || null`.
```

- [ ] **Step 2: Add server validation in `routes.rs`**

In `crates/oxipage-ext-scraps/src/routes.rs`, in `create_manual` and `update`:

```rust
if !oxipage_core::validation::is_http_url(&input.source_url) {
    return Err(ApiError::validation("source_url", "source_url must be an http(s) URL"));
}
if !crate::model::normalize_source(input.source.as_deref()).is_ascii() {
    return Err(ApiError::validation("source", "source must be hackernews|geeknews|manual"));
}
if let Some(og) = &input.og_image_url
    && !og.is_empty()
    && !oxipage_core::validation::is_image_value(og)
{
    return Err(ApiError::validation("og_image_url", "og_image_url must be an http(s) URL or site media path"));
}
```

- [ ] **Step 3: Type-check + build + smoke + commit**

Run: `cd web && npx tsc --noEmit`; `cargo build -p oxipage-ext-scraps`; smoke creating a manual scrap.

```bash
git add web/src/admin/content/ScrapsTab.tsx crates/oxipage-ext-scraps/
git commit -m "feat(scraps): ImageField override, TagInput, read-only/editable split"
```

---

### Task 22: Blog server validation (title required, lang in site languages)

**Files:**
- Modify: `crates/oxipage-ext-blog/src/routes.rs`

**Interfaces:**
- Produces: blog create rejects empty title and rejects `lang` outside the site's enabled languages.

The foundation plan keeps `MutableSiteSettings` unchanged; the blog per-site handler reaches it via the post-foundation `SiteContext`. By the time this task runs, the console middleware that builds `SiteScopedDb` exposes a `settings: Arc<RwLock<MutableSiteSettings>>` field. (If the foundation plan does not add that field, this task adds a one-liner `settings: Arc<RwLock<MutableSiteSettings>>` on `SiteScopedDb` and wires it in the per-site middleware — no design change.)

- [ ] **Step 1: Tighten blog create validation**

In `crates/oxipage-ext-blog/src/routes.rs`, in `create`, add:

```rust
if input.title.trim().is_empty() {
    return Err(ApiError::validation("title", "title must not be empty"));
}
let enabled: std::collections::BTreeSet<String> = pool
    .settings
    .read()
    .await
    .site
    .languages
    .iter()
    .map(|l| l.code.clone())
    .collect();
if !enabled.contains(&input.lang) {
    return Err(ApiError::validation("lang", "lang is not enabled for this site"));
}
```

(Engineer: if `SiteScopedDb` doesn't yet carry `settings`, mirror the foundation contract by adding `pub settings: Arc<RwLock<MutableSiteSettings>>` to `state.rs::SiteScopedDb` and propagating it from the per-site middleware.)

- [ ] **Step 2: Build + commit**

Run: `cargo build -p oxipage-ext-blog`. Expected: success.

```bash
git add crates/oxipage-ext-blog/
git commit -m "feat(blog): server validates title and lang against settings.site.languages"
```

---

### Task 23: Profile `expected_updated_at=""` first-write smoke

After Task 11 lands, verify the route handles `""` for the initial no-row case.

- [ ] **Step 1: Smoke**

Run: `cd web && bun run dev`; create a fresh site; open Profile tab; fill Save; verify row created.

(No code change expected; verified by Task 11 step 4 `input.expected_updated_at.is_empty()` guard.)

---

## Self-Review

**1. Spec coverage:**
- §3 Renderer reuse → Task 8 (BlogPostView + BlogPostCard), Task 9 (ProfileView), Task 10 (BookCard/MovieCard/NovelCard/LinkCard/ScrapCard/ProjectCard + ProjectView).
- §4 Editor/preview layout → Task 5 (`EditorPreviewDrawer` desktop 2-pane, mobile tabs, theme scope) + Task 6 (`DraftPreviewPane`).
- §5 Profile Admin — eighth tab, full singleton, GET-before-Save, 409 → Task 11 (server) + Task 12 (ProfileTab + ContentPage wiring).
- §6.1 Blog — TagInput, language from config, draft preview → Task 15 + server validation Task 22.
- §6.2 Projects — links repeater, started/ended dates, ImageField screenshots, KO/EN alt, validate, atomic reorder → Task 17 + §8 validation Task 14 + reorder Task 14.
- §6.3 Links — ImageField thumbnail, TagInput, http(s)/media validation → Task 20.
- §6.4 Movies — TMDB search wired, series cover ImageField, remove `any` → Task 18.
- §6.5 Books — public card, ImageField cover, MarkdownEditor reviews, status enum fix, ISBN-13 + date validation → Task 16.
- §6.6 Novels — public card, ImageField cover, TagInput, chapter title required, atomic reorder → Task 19 + Task 13.
- §6.7 Scraps — public card, OG override via ImageField, TagInput, read-only/editable, source http(s) + enum → Task 21.
- §7 Atomic reorder APIs → Task 13 (novels), Task 14 (projects + core validators).
- §8 Validation contract — `ApiValidationError`, `DrawerField.error`, shared client/server validators → Task 1, Task 2, Task 4, Task 14.
- §9 Editing-state safety (dirty close guard, failed mutation preserves form) → Task 5 (`EditorPreviewDrawer` dirty prop, close confirm) + per-tab `disabled={save.isPending || !dirty}` in Tasks 12/15/16/17/18/19/20/21.
- §10 API/client additions → Task 11 (Profile), Tasks 13/14 (orders), Tasks 15/16/17/18/19 (TagInput, ImageField, search).

**2. Placeholder scan:** No "TBD", "TODO", or "fill in". Every step has actual code or exact command. The `todo!()` and helper stubs in tests are intentional scaffolds for the engineer to plug the same `AppState`/`SiteScopedDb` helpers used by their existing tests.

**3. Type consistency:**
- `BlogPostData` exported from `BlogPostView.tsx` (Task 8); reused by BlogTab (Task 15) and DraftPreviewPane.
- `ProfileData` exported from `ProfileView.tsx` (Task 9); reused by ProfileTab (Task 12) and the public page.
- `BookCardData`/`MovieCardData`/`NovelCardData`/`LinkCardData`/`ScrapCardData`/`ProjectCardData`/`ProjectViewData` (Task 10) match the shape of the corresponding shared `fetch*` results.
- Server `ProfileInput.expected_updated_at` (Task 11) ↔ client `ProfileTab` payload (Task 12).
- `reorderChapters(slug, novelSlug, ids)` ↔ PUT `/novels/{slug}/chapters/order` (Task 13).
- `reorderScreenshots(slug, projectSlug, ids)` ↔ PUT `/projects/{slug}/screenshots/order` (Task 14).
- `ApiValidationError` ↔ server `{ error: { field, message, code } }` (Task 1).
- `validation.rs` server functions (Task 14) mirror `web/.../validation.ts` (Task 2) by name and semantics.

**Gaps caught during self-review:**
- The shared `web/src/admin/shared/ui/ImageField.tsx` is produced by the preview/media plan; this plan only adds `error` to its props (Task 7).
- `AssetResolverProvider` and `PublicThemeScope` are referenced from peer plans (Task 5) — not redefined here, so no conflict.
- Post-foundation `SiteContext` exposes `settings: Arc<RwLock<MutableSiteSettings>>`; per-site handlers reach `site.languages` via `pool.settings.read().await.site.languages`. Blog lang validation (Task 22) reads through that path. If the foundation plan does not propagate `settings` into `SiteScopedDb`, Task 22 adds the one-liner.
- The Profile crate keeps its local `validate_email`/`validate_year_range` (Task 11) to stay self-contained; the core validators in Task 14 cover all other extensions to avoid drift.
- The "Profile GET returns updated_at" requirement (spec §5) is already satisfied — `Profile` model includes `updated_at`; the Admin form reads it on load.

**No spec requirement lacks a task.**