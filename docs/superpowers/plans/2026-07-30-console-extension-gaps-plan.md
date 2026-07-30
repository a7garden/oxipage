# Console Extension Gaps — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Complete 4 extension feature gaps (Novels chapters, Movies series groups, Projects screenshots, WASM registry install).

**Architecture:** Frontend-heavy (70%) + small backend fills (30%). Sub-resources managed via parent Drawer accordion sections. Movies series groups use a `[Movies|Series]` sub-toggle.

**Tech Stack:** Rust (axum, sqlx), TypeScript/React (TanStack Query, Tailwind v4)

## Global Constraints

- API prefix: `/api/console` (per-site via `/s/{slug}/…`)
- All new client functions: `siteScopedFetch` + `jsonOrThrow` (not the flat `contentClient`)
- Table-name interpolation must pass `is_safe_ident`
- Reuse existing UI primitives (Drawer, DrawerField, Button, Badge, Input, Textarea, Skeleton, EmptyState)
- Loading/empty/error states per shell-redesign global constraints
- Optimistic updates for reorder; onError rollback + onSettled invalidate

---

### Task 1: Backend — Movies series group update/delete

**Files:**
- Modify: `crates/oxipage-ext-movies/src/model.rs` (add `SeriesGroupPatch`)
- Modify: `crates/oxipage-ext-movies/src/repo.rs` (add `update_group`, `delete_group`)
- Modify: `crates/oxipage-ext-movies/src/routes.rs` (add `update_group`, `delete_group` handlers)
- Modify: `crates/oxipage-ext-movies/src/lib.rs` (mount `PATCH/DELETE /series/{slug}`)

**Interfaces:**
- Produces: `repo::update_group(pool, slug, patch) -> Result<Option<SeriesGroup>>`
- Produces: `repo::delete_group(pool, slug) -> Result<bool>` (NULLs members' `series_group_id`)
- Produces: `routes::update_group` / `routes::delete_group` handlers

- [ ] **Step 1: Add `SeriesGroupPatch` model**

```rust
// model.rs
#[derive(Debug, Clone, Deserialize, Default)]
pub struct SeriesGroupPatch {
    pub title_ko: Option<String>,
    pub title_en: Option<String>,
    pub cover_image: Option<String>,
    pub group_rating: Option<i8>,
    pub group_review_ko: Option<String>,
    pub group_review_en: Option<String>,
}
```

- [ ] **Step 2: Add `repo::update_group`**

```rust
// repo.rs — after list_groups
pub async fn update_group(
    pool: &SqlitePool,
    slug: &str,
    patch: &SeriesGroupPatch,
) -> anyhow::Result<Option<SeriesGroup>> {
    // SET each field if Some, mirroring MovieEntry::update_entry pattern (lines 244-334)
    // existing slug check → UPDATE series_group SET ... WHERE slug = ?
    // Return updated row or None if not found
}
```
Pattern: build SET parts from `patch` fields, join with commas, execute UPDATE, return `find_group_by_slug`.

- [ ] **Step 3: Add `repo::delete_group`**

```rust
// repo.rs
pub async fn delete_group(pool: &SqlitePool, slug: &str) -> anyhow::Result<bool> {
    // Find group id from slug
    // UPDATE movie_entry SET series_group_id = NULL WHERE series_group_id = ?
    // DELETE FROM series_group WHERE id = ?
    // Return true if deleted, false if not found
}
```

- [ ] **Step 4: Add route handlers**

```rust
// routes.rs
pub async fn update_group(
    Extension(pool): Extension<SiteScopedDb>,
    Path(slug): Path<String>,
    Json(patch): Json<SeriesGroupPatch>,
) -> Result<Json<DataEnvelope<SeriesGroup>>, ApiError> {
    let group = repo::update_group(&pool.0, &slug, &patch)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| not_found_group(&slug))?;
    Ok(Json(DataEnvelope { data: group }))
}

pub async fn delete_group(
    Extension(pool): Extension<SiteScopedDb>,
    Path(slug): Path<String>,
) -> Result<Json<DataEnvelope<serde_json::Value>>, ApiError> {
    let removed = repo::delete_group(&pool.0, &slug)
        .await
        .map_err(ApiError::from)?;
    if !removed { return Err(not_found_group(&slug)); }
    Ok(Json(DataEnvelope { data: serde_json::json!({"removed": true}) }))
}
```

- [ ] **Step 5: Mount routes in lib.rs**

```rust
// lib.rs routes() — add after the existing GET /series/{slug}
.route("/series/{slug}", get(routes::show_group)
    .patch(routes::update_group)
    .delete(routes::delete_group))
```
Fix: change the existing `.route("/series/{slug}", get(routes::show_group))` to `.route("/series/{slug}", get(routes::show_group).patch(routes::update_group).delete(routes::delete_group))`

- [ ] **Step 6: Build and test**

Run: `cargo check -p oxipage-ext-movies`
Expected: compiles cleanly.

---

### Task 2: Backend — Projects screenshot update

**Files:**
- Modify: `crates/oxipage-ext-projects/src/model.rs` (add `ScreenshotPatch`)
- Modify: `crates/oxipage-ext-projects/src/repo.rs` (add `update_screenshot`)
- Modify: `crates/oxipage-ext-projects/src/routes.rs` (add `update_screenshot` handler)
- Modify: `crates/oxipage-ext-projects/src/lib.rs` (mount `PATCH /{slug}/screenshots/{sid}`)

**Interfaces:**
- Produces: `repo::update_screenshot(pool, project_slug, sid, patch) -> Result<Option<Screenshot>>`

- [ ] **Step 1: Add `ScreenshotPatch` model**

```rust
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ScreenshotPatch {
    pub alt_ko: Option<String>,
    pub alt_en: Option<String>,
    pub display_order: Option<i32>,
}
```

- [ ] **Step 2: Add `repo::update_screenshot`**

```rust
// repo.rs — after delete_screenshot
pub async fn update_screenshot(
    pool: &SqlitePool,
    project_slug: &str,
    sid: i64,
    patch: &ScreenshotPatch,
) -> anyhow::Result<Option<Screenshot>> {
    // Find project_id from slug
    // Build SET from patch fields
    // UPDATE screenshot SET ... WHERE id = ? AND project_id = ?
    // Return updated row or None
}
```

- [ ] **Step 3: Add route handler + mount**

```rust
// routes.rs
pub async fn update_screenshot(
    Extension(pool): Extension<SiteScopedDb>,
    Path((slug, sid)): Path<(String, i64)>,
    Json(patch): Json<ScreenshotPatch>,
) -> Result<Json<DataEnvelope<Screenshot>>, ApiError> { ... }

// lib.rs — add next to the existing DELETE:
.route("/{slug}/screenshots/{sid}",
    axum::routing::delete(routes::delete_screenshot))
```

Wait — the existing route is `DELETE` only for `/{slug}/screenshots/{sid}`. Change to:
```rust
.route("/{slug}/screenshots/{sid}",
    axum::routing::delete(routes::delete_screenshot)
        .patch(routes::update_screenshot))
```

- [ ] **Step 4: Build check**

`cargo check -p oxipage-ext-projects`

---

### Task 3: Backend — WASM registry list endpoint

**Files:**
- Modify: `crates/oxipage-core/src/http.rs`

**Interfaces:**
- Produces: `GET /api/console/extensions/registry` → `{ data: RegistryEntry[] }`
- Consumes: existing `REGISTRY_INDEX_JSON` (embedded `_registry.json`), `state.registry.find()`

- [ ] **Step 1: Add registry_list handler**

```rust
// http.rs — near extension_install (~line 640 area)

#[derive(Serialize)]
struct RegistryListEntry {
    name: String,
    runtime_loadable: bool,
    installed: bool,
    source: &'static str,  // "embedded" or "remote"
}

async fn registry_list(
    State(state): State<AppState>,
) -> Result<Json<DataEnvelope<Vec<RegistryListEntry>>>, ApiError> {
    let index: RegistryIndex = serde_json::from_str(REGISTRY_INDEX_JSON)
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "registry_error", ...))?;
    let entries: Vec<RegistryListEntry> = index.extensions
        .into_iter()
        .filter(|e| e.runtime_loadable)
        .map(|e| RegistryListEntry {
            name: e.name,
            runtime_loadable: true,
            installed: state.registry.find(&e.name).is_some(),
            source: if e.name == "wasm-demo" { "embedded" } else { "remote" },
        })
        .collect();
    Ok(Json(DataEnvelope { data: entries }))
}
```

- [ ] **Step 2: Mount route**

```rust
// In the api router, next to /extensions/install:
.route("/extensions/registry", axum::routing::get(registry_list))
```

- [ ] **Step 3: Build check**

`cargo check -p oxipage-core`

---

### Task 4: Client — Sub-resource API functions

**Files:**
- Modify: `web/src/admin/shared/api.ts`

- [ ] **Step 1: Add chapters client fns**

```ts
// api.ts — after contentClient

export interface NovelChapter {
  id: number; novel_id: number; chapter_order: number;
  title: string; body: string; char_count: number;
  published_at: string | null;
  created_at: string; updated_at: string;
}

export async function listChapters(slug: string, novelSlug: string, draft = false): Promise<NovelChapter[]> {
  const path = draft ? `/novels/${novelSlug}/chapters/draft` : `/novels/${novelSlug}/chapters`;
  const res = await siteScopedFetch(slug, path);
  if (!res.ok) return [];
  const json = await res.json() as { data?: NovelChapter[] };
  return json.data ?? [];
}

export async function createChapter(
  slug: string, novelSlug: string, input: { chapter_order: number; title: string; body?: string }
): Promise<NovelChapter> {
  const res = await siteScopedFetch(slug, `/novels/${novelSlug}/chapters`, {
    method: "POST", headers: { "content-type": "application/json" },
    body: JSON.stringify(input),
  });
  return jsonOrThrow<{ data: NovelChapter }>(res).then(j => j.data);
}

export async function updateChapter(
  slug: string, novelSlug: string, order: number, patch: { title?: string; body?: string; chapter_order?: number }
): Promise<NovelChapter> {
  const res = await siteScopedFetch(slug, `/novels/${novelSlug}/chapters/${order}`, {
    method: "PATCH", headers: { "content-type": "application/json" },
    body: JSON.stringify(patch),
  });
  return jsonOrThrow<{ data: NovelChapter }>(res).then(j => j.data);
}

export async function deleteChapter(slug: string, novelSlug: string, order: number): Promise<void> {
  const res = await siteScopedFetch(slug, `/novels/${novelSlug}/chapters/${order}`, { method: "DELETE" });
  if (!res.ok) throw new Error(`HTTP ${res.status}`);
}

export async function publishChapter(slug: string, novelSlug: string, order: number): Promise<NovelChapter> {
  const res = await siteScopedFetch(slug, `/novels/${novelSlug}/chapters/${order}/publish`, { method: "POST" });
  return jsonOrThrow<{ data: NovelChapter }>(res).then(j => j.data);
}
```

- [ ] **Step 2: Add series + screenshots + registry client fns**

```ts
// series
export interface SeriesGroup { id: number; slug: string; title_ko: string | null; title_en: string | null; cover_image: string | null; group_rating: number | null; created_at: string; updated_at: string; }
export interface SeriesGroupDetail { group: SeriesGroup; entries: MovieEntry[]; }

export async function listSeries(slug: string): Promise<SeriesGroup[]> {
  const res = await siteScopedFetch(slug, "/movies/series");
  if (!res.ok) return [];
  const json = await res.json() as { data?: SeriesGroup[] };
  return json.data ?? [];
}

export async function createSeries(slug: string, input: { title_ko?: string; title_en?: string }): Promise<SeriesGroup> {
  const res = await siteScopedFetch(slug, "/movies/series", { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify(input) });
  return jsonOrThrow<{ data: SeriesGroup }>(res).then(j => j.data);
}

export async function showSeries(slug: string, groupSlug: string): Promise<SeriesGroupDetail | null> {
  const res = await siteScopedFetch(slug, `/movies/series/${groupSlug}`);
  if (!res.ok) return null;
  return jsonOrThrow<{ data: SeriesGroupDetail }>(res).then(j => j.data);
}

export async function updateSeries(slug: string, groupSlug: string, patch: Partial<{ title_ko: string; title_en: string; cover_image: string; group_rating: number }>): Promise<SeriesGroup> {
  const res = await siteScopedFetch(slug, `/movies/series/${groupSlug}`, { method: "PATCH", headers: { "content-type": "application/json" }, body: JSON.stringify(patch) });
  return jsonOrThrow<{ data: SeriesGroup }>(res).then(j => j.data);
}

export async function deleteSeries(slug: string, groupSlug: string): Promise<void> {
  const res = await siteScopedFetch(slug, `/movies/series/${groupSlug}`, { method: "DELETE" });
  if (!res.ok) throw new Error(`HTTP ${res.status}`);
}

// screenshots
export interface Screenshot { id: number; project_id: number; url: string; alt_ko: string | null; alt_en: string | null; display_order: number; created_at: string; }

export async function addScreenshot(slug: string, projectSlug: string, input: { url: string; alt_ko?: string; alt_en?: string; display_order?: number }): Promise<Screenshot> {
  const res = await siteScopedFetch(slug, `/projects/${projectSlug}/screenshots`, { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify(input) });
  return jsonOrThrow<{ data: Screenshot }>(res).then(j => j.data);
}

export async function updateScreenshot(slug: string, projectSlug: string, sid: number, patch: { alt_ko?: string; alt_en?: string; display_order?: number }): Promise<Screenshot> {
  const res = await siteScopedFetch(slug, `/projects/${projectSlug}/screenshots/${sid}`, { method: "PATCH", headers: { "content-type": "application/json" }, body: JSON.stringify(patch) });
  return jsonOrThrow<{ data: Screenshot }>(res).then(j => j.data);
}

export async function deleteScreenshot(slug: string, projectSlug: string, sid: number): Promise<void> {
  const res = await siteScopedFetch(slug, `/projects/${projectSlug}/screenshots/${sid}`, { method: "DELETE" });
  if (!res.ok) throw new Error(`HTTP ${res.status}`);
}

// registry
export interface RegistryEntry { name: string; runtime_loadable: boolean; installed: boolean; source: string; }

export async function listRegistry(): Promise<RegistryEntry[]> {
  const res = await fetch(`/api/console/extensions/registry`);
  if (!res.ok) return [];
  const json = await res.json() as { data?: RegistryEntry[] };
  return json.data ?? [];
}

export async function installExtension(name: string): Promise<{ name: string; activated: boolean; note?: string }> {
  const res = await fetch(`/api/console/extensions/install`, { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ name }) });
  return jsonOrThrow<{ data: { name: string; activated: boolean; note?: string } }>(res).then(j => j.data);
}
```

- [ ] **Step 3: TypeScript check**

Run: `cd web && npx tsc --noEmit`
Expected: no errors.

---

### Task 5: Frontend — NovelsTab chapters accordion

**Files:**
- Modify: `web/src/admin/content/NovelsTab.tsx`

- [ ] **Step 1: Add chapter state and accordion in the edit Drawer**

Inside the novel edit drawer (between the form fields and the footer buttons), add a collapsible "Chapters" section:

```tsx
// After the existing form fields, before the footer:
<div className="border-t border-line pt-4 mt-4">
  <button
    onClick={() => setShowChapters(!showChapters)}
    className="flex items-center gap-2 text-sm font-semibold text-foreground mb-3"
  >
    <ChevronDown size={14} className={showChapters ? "" : "-rotate-90 transition-transform"} />
    Chapters {chapters.length > 0 && `(${chapters.length})`}
  </button>
  {showChapters && (
    <div className="space-y-1 mb-3">
      {chapters.map((ch, i) => (
        <div key={ch.id} className="flex items-center gap-2 px-2 py-1.5 rounded border border-line text-sm">
          <span className="text-muted w-6 shrink-0 text-center">{ch.chapter_order}</span>
          <span className="flex-1 truncate">{ch.title}</span>
          <Badge variant={ch.published_at ? "default" : "outline"} className="text-[10px] px-1.5">
            {ch.published_at ? "Published" : "Draft"}
          </Badge>
          <button onClick={() => /* toggle edit inline */} className="text-muted hover:text-foreground">
            <Pencil size={14} />
          </button>
          <button onClick={() => /* ↑ reorder */} disabled={i === 0} className="text-muted hover:text-foreground disabled:opacity-30">
            <ChevronUp size={14} />
          </button>
          <button onClick={() => /* ↓ reorder */} disabled={i === chapters.length - 1} className="text-muted hover:text-foreground disabled:opacity-30">
            <ChevronDown size={14} />
          </button>
          <button onClick={() => deleteChapterMutation.mutate(ch.chapter_order)} className="text-red-500 hover:text-red-600">
            <Trash2 size={14} />
          </button>
        </div>
      ))}
    </div>
  )}
  <button
    onClick={() => { setEditingChapter("new"); setChapterForm({ chapter_order: (chapters.length > 0 ? Math.max(...chapters.map(c => c.chapter_order)) : 0) + 1, title: "", body: "" }); }}
    className="flex items-center gap-1 text-xs text-muted hover:text-foreground"
  >
    <Plus size={12} /> Add Chapter
  </button>
</div>
```

- [ ] **Step 2: Wire queries and mutations**

```tsx
// In NovelsTab, when editing is an object (not "new"):
const novelSlug = typeof editing === "object" ? editing.slug : undefined;
const { data: chapters = [] } = useQuery({
  queryKey: ["site", slug, "novels", novelSlug, "chapters"],
  queryFn: () => listChapters(slug!, novelSlug!),
  enabled: !!novelSlug && showChapters,
});

const createChapterMutation = useMutation({
  mutationFn: (input: { chapter_order: number; title: string; body: string }) =>
    createChapter(slug!, novelSlug!, input),
  onSuccess: () => qc.invalidateQueries({ queryKey: ["site", slug, "novels", novelSlug, "chapters"] }),
});

// + updateChapterMutation (reorder)
// + deleteChapterMutation
```

- [ ] **Step 3: Add inline chapter editing (new + edit expansion)**

When "Add Chapter" is clicked (or an existing chapter's edit icon), show an inline editor below the list:

```tsx
{editingChapter && (
  <div className="border border-line rounded p-3 mb-3 space-y-2 bg-surface/30">
    <Input
      placeholder="Chapter title"
      value={chapterForm.title}
      onChange={e => setChapterForm(f => ({ ...f, title: e.target.value }))}
    />
    <Textarea
      placeholder="Chapter body"
      rows={6}
      value={chapterForm.body}
      onChange={e => setChapterForm(f => ({ ...f, body: e.target.value }))}
    />
    <div className="flex gap-2 justify-end">
      <Button variant="outline" size="sm" onClick={() => setEditingChapter(null)}>Cancel</Button>
      <Button size="sm" onClick={() => {
        if (editingChapter === "new") createChapterMutation.mutate(chapterForm);
        else updateChapterMutation.mutate({ order: editingChapter, patch: chapterForm });
      }}>
        {editingChapter === "new" ? "Add" : "Save"}
      </Button>
    </div>
  </div>
)}
```

- [ ] **Step 4: TypeScript check**

Run: `cd web && npx tsc --noEmit`
Expected: no errors.

---

### Task 6: Frontend — MoviesTab series sub-view

**Files:**
- Modify: `web/src/admin/content/MoviesTab.tsx`

- [ ] **Step 1: Add `[Movies | Series]` toggle and series state**

```tsx
const [tab, setTab] = useState<"movies" | "series">("movies");

// Before the table:
<div className="flex gap-4 mb-4">
  <button
    onClick={() => setTab("movies")}
    className={`text-sm font-medium pb-1 border-b-2 ${tab === "movies" ? "border-[#22c55e] text-foreground" : "border-transparent text-muted"}`}
  >Movies</button>
  <button
    onClick={() => setTab("series")}
    className={`text-sm font-medium pb-1 border-b-2 ${tab === "series" ? "border-[#22c55e] text-foreground" : "border-transparent text-muted"}`}
  >Series</button>
</div>
```

- [ ] **Step 2: Render Series view when tab === "series"**

```tsx
{tab === "series" ? (
  <SeriesView slug={slug} />
) : (
  // existing movie entries table...
)}
```

- [ ] **Step 3: Create SeriesView sub-component**

```tsx
// In the same file, a local component:
function SeriesView({ slug }: { slug: string }) {
  const qc = useQueryClient();
  const { data: groups = [] } = useQuery({
    queryKey: ["site", slug, "movies", "series"],
    queryFn: () => listSeries(slug!),
    enabled: !!slug,
  });

  const [editing, setEditing] = useState<null | SeriesGroup | "new">(null);
  const [selected, setSelected] = useState<null | SeriesGroup>(null);
  const detailQuery = useQuery({
    queryKey: ["site", slug, "movies", "series", selected?.slug],
    queryFn: () => showSeries(slug!, selected!.slug),
    enabled: !!selected,
  });

  // Series CRUD mutations...
  // Render: group list + create/edit Drawer + detail panel with members
}
```

- [ ] **Step 4: Add "Series" field to movie entry edit Drawer**

```tsx
// In the existing movie edit Drawer (form fields area):
const { data: groups = [] } = useQuery({
  queryKey: ["site", slug, "movies", "series"],
  queryFn: () => listSeries(slug!),
  enabled: !!slug,
});

// Add after review fields:
<DrawerField label="Series">
  <select
    className="w-full border border-line rounded px-2 py-1.5 text-sm bg-surface"
    value={form.series_group_slug ?? ""}
    onChange={e => setForm(f => ({ ...f, series_group_slug: e.target.value || null }))}
  >
    <option value="">None</option>
    {groups.map(g => <option key={g.slug} value={g.slug}>{g.title_ko ?? g.title_en ?? g.slug}</option>)}
  </select>
</DrawerField>
<DrawerField label="Series Order">
  <input
    type="number" min={0}
    className="w-full border border-line rounded px-2 py-1.5 text-sm bg-surface"
    value={form.series_order ?? ""}
    onChange={e => setForm(f => ({ ...f, series_order: e.target.value ? Number(e.target.value) : null }))}
  />
</DrawerField>
```

- [ ] **Step 5: TypeScript check**

---

### Task 7: Frontend — ProjectsTab screenshots accordion

**Files:**
- Modify: `web/src/admin/content/ProjectsTab.tsx`

- [ ] **Step 1: Extract screenshots from project detail**

`showExtension<ProjectDetail>(slug, "projects", projectSlug)` returns `{ project, screenshots }` — extract the screenshots array from the `show` response. Add a state:

```tsx
const [showScreenshots, setShowScreenshots] = useState(false);
const projectDetail = useQuery({
  queryKey: ["site", slug, "projects", editing && typeof editing === "object" ? editing.slug : undefined],
  queryFn: () => showExtension<ProjectDetail>(slug!, "projects", (editing as Project).slug),
  enabled: !!slug && typeof editing === "object" && editing !== null,
});
const screenshots = (projectDetail.data as ProjectDetail | undefined)?.screenshots ?? [];
```

- [ ] **Step 2: Add screenshots accordion in Drawer**

Same pattern as NovelsTab: collapsible "Screenshots" section with thumbnail preview, alt edit, ↑↓ reorder, delete, add.

```tsx
<DrawerField label="Screenshots">
  <div className="space-y-2">
    {screenshots.map((s, i) => (
      <div key={s.id} className="flex items-center gap-2 p-2 border border-line rounded">
        <img src={s.url} alt="" className="size-10 rounded object-cover shrink-0" />
        <input className="flex-1 text-xs border border-line rounded px-1 py-0.5" value={s.alt_ko ?? ""} onChange={...} />
        <input type="number" className="w-12 text-xs border border-line rounded px-1 py-0.5" value={s.display_order} onChange={...} />
        <button onClick={() => /* ↑ */} disabled={i===0}><ChevronUp size={14}/></button>
        <button onClick={() => /* ↓ */} disabled={i===screenshots.length-1}><ChevronDown size={14}/></button>
        <button onClick={() => deleteScreenshotMutation.mutate(s.id)} className="text-red-500"><Trash2 size={14}/></button>
      </div>
    ))}
    <Button variant="outline" size="sm" onClick={() => /* add screenshot prompt */}>
      <Plus size={12} className="mr-1" /> Add Screenshot
    </Button>
  </div>
</DrawerField>
```

- [ ] **Step 3: Wire mutations**

```tsx
const addScreenshotMutation = useMutation({
  mutationFn: (input: { url: string; alt_ko?: string; alt_en?: string }) =>
    addScreenshot(slug!, projectSlug, input),
  onSuccess: () => projectDetail.refetch(),
});

const deleteScreenshotMutation = useMutation({
  mutationFn: (sid: number) => deleteScreenshot(slug!, projectSlug, sid),
  onSuccess: () => projectDetail.refetch(),
});
```

- [ ] **Step 4: TypeScript check**

---

### Task 8: Frontend — ExtensionsPage available from registry

**Files:**
- Modify: `web/src/admin/extensions/ExtensionsPage.tsx`

- [ ] **Step 1: Add registry query + install mutation below the installed grid**

```tsx
// In ExtensionsPage, after the installed section:
const { data: registry = [] } = useQuery({
  queryKey: ["extensions", "registry"],
  queryFn: listRegistry,
});

const installExt = useMutation({
  mutationFn: (name: string) => installExtension(name),
  onSuccess: (result, name) => {
    qc.invalidateQueries({ queryKey: ["extensions", "registry"] });
    qc.invalidateQueries({ queryKey: ["site", slug, "extensions"] });
    setInstallNote(`${name}: ${result.activated ? "Activated" : result.note ?? "Restart to activate"}`);
  },
});

// Registry section:
{(registry?.filter(r => !r.installed) ?? []).length > 0 && (
  <>
    <div className="text-xs font-semibold uppercase tracking-wider text-muted mb-3 mt-6">
      Available from Registry
    </div>
    <div className="grid grid-cols-2 gap-3">
      {registry.filter(r => !r.installed).map(entry => (
        <div key={entry.name} className="border border-line rounded-lg p-3 flex items-center gap-3">
          <div className="size-9 rounded-lg bg-[#e0f2fe] text-[#0369a1] flex items-center justify-center text-base font-bold shrink-0">
            {entry.name[0].toUpperCase()}
          </div>
          <div className="flex-1 min-w-0">
            <div className="text-sm font-medium">{entry.name}</div>
            <div className="text-xs text-muted truncate">{entry.source}</div>
          </div>
          <Button
            size="sm"
            onClick={() => installExt.mutate(entry.name)}
            disabled={installExt.isPending}
          >Install</Button>
        </div>
      ))}
    </div>
    {installNote && <p className="text-xs text-muted mt-2">{installNote}</p>}
  </>
)}
```

- [ ] **Step 2: TypeScript check**

---

### Task 9: Wire everything and smoke test

- [ ] **Step 1: Full build check**

```bash
cargo check 2>&1
cd web && npx tsc --noEmit 2>&1
```

Expected: both pass.

- [ ] **Step 2: Manual smoke**

Run `oxipage console`, navigate each extension tab:
1. Novels: open a novel → add chapter with title+body → verify auto-order and char_count → edit chapter → reorder (↑↓) → delete chapter
2. Movies: switch to Series tab → create series → edit a movie entry → assign to series with order → switch back to Series → view member list → unassign member → delete series
3. Projects: edit a project → add screenshot URL → verify preview → reorder → edit alt → delete
4. Extensions: check "Available from Registry" shows wasm-demo → click Install → see activation feedback
