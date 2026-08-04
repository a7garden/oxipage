# Static Site Generator Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use subagent-driven-development (recommended) or executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the Static Site Generator (v2) pivot — `BuildExt` trait, build pipeline, CLI commands (build/deploy/query/schema/cache refresh), React data layer switch, and preview mode.

**Architecture:** Three new subsystems layered on the existing code: `BuildExt` trait (oxibuilder-core) + per-extension implementations → rayon parallel build pipeline → CLI commands (build → deploy). The React SPA switches from live API to static JSON files at build time. The management server (`oxibuilder serve`) stays unchanged; a `--preview` mode serves `out/` locally.

**Tech Stack:** Rust Edition 2024, rayon (parallel iteration), sqlx (direct DB reads), serde (JSON dump), git2 or `gh` CLI (GitHub Pages deploy), git worktree, clap derive (CLI subcommands).

**Design spec:** `docs/superpowers/specs/2026-07-28-static-site-generator-design.md`

## Global Constraints

- Edition 2024, all existing lints/clippy rules apply
- `cargo test --workspace` must pass after every task
- `cargo clippy --all-targets -- -D warnings` must be clean after every task
- No new async dependencies for build path (build is CPU-bound sync)
- Korean CLI output for Korean messages, English for --json
- All existing `Extension` trait code remains unchanged
- `BuildExt` is a separate trait, never merged into `Extension`

---

### Task 1: Define BuildExt trait in oxibuilder-core

**Files:**
- Modify: `crates/oxibuilder-core/src/extension.rs`
- Create: `crates/oxibuilder-core/src/builder.rs`

**Interfaces:**
- Produces: `pub trait BuildExt`, `pub struct StaticPage`, `pub struct SearchDoc`, `pub struct BuildOutput`

**Context:** Build pipeline types and trait definition.

- [ ] **Step 1: Create `crates/oxibuilder-core/src/builder.rs`**

```rust
use crate::db;
use serde::Serialize;
use std::error::Error;

/// A single static HTML page produced during build.
pub struct StaticPage {
    /// Relative URL path, e.g. "blog/hello-world/index.html"
    pub path: String,
    /// Full HTML content (including <!DOCTYPE html>, OG metas, etc.)
    pub content: String,
}

/// A document for the client-side search index.
pub struct SearchDoc {
    /// Unique document id, e.g. "blog/hello-world"
    pub id: String,
    pub title: String,
    pub body_preview: String,
    pub r#type: String,
    pub url: String,
    pub published_at: Option<String>,
}

/// Output from a single extension's build.
pub struct ExtBuildOutput {
    pub ext_id: String,
    pub pages: Vec<StaticPage>,
    pub data: Box<dyn erased_serde::Serialize + Send>,
    pub search_docs: Vec<SearchDoc>,
}

/// Aggregated build output.
pub struct BuildOutput {
    pub pages: Vec<StaticPage>,
    pub search_docs: Vec<SearchDoc>,
    pub extensions_data: Vec<(String, Box<dyn erased_serde::Serialize + Send>)>,
}

/// Each extension implements this to participate in site generation.
/// Build is CPU-bound and synchronous — no async needed.
pub trait BuildExt: Send + Sync {
    type Error: Error + Send + 'static;

    /// Generate static HTML pages for published content.
    /// URL path convention: `{ext_id}/{slug}/index.html`
    fn build_pages(&self, db: &db::Pool) -> Result<Vec<StaticPage>, Self::Error>;

    /// Generate client-side data as a serializeable object.
    /// Will be written to `out/data/{ext_id}.json`.
    fn build_data(&self, db: &db::Pool) -> Result<Box<dyn erased_serde::Serialize + Send>, Self::Error>;

    /// Generate search index documents for this extension's content.
    fn build_search_docs(&self, db: &db::Pool) -> Result<Vec<SearchDoc>, Self::Error>;
}
```

Use `erased_serde` for dynamic dispatch of `build_data` return values. Add it to workspace deps:
```toml
# workspace Cargo.toml
erased-serde = "0.4"
```

- [ ] **Step 2: Add `erased-serde` dependency to workspace**

Edit `Cargo.toml` workspace dependencies:
```toml
erased-serde = "0.4"
```

Add to `oxibuilder-core/Cargo.toml`:
```toml
erased-serde.workspace = true
rayon.workspace = true
```

- [ ] **Step 3: Check it compiles**

```bash
cargo check -p oxibuilder-core
```

- [ ] **Step 4: Git commit**

```
git add Cargo.toml crates/oxibuilder-core/Cargo.toml crates/oxibuilder-core/src/extension.rs crates/oxibuilder-core/src/builder.rs
git commit -m "feat(core): add BuildExt trait and build pipeline types"
```

---

### Task 2: Build pipeline (registry + rayon dispatch)

**Files:**
- Modify: `crates/oxibuilder-core/src/registry.rs` (add `RegisterBuild` method)
- Create: `crates/oxibuilder-core/src/build.rs`

**Interfaces:**
- Consumes: `BuildExt` trait, `BuildOutput`
- Produces: `pub fn build_site(db: &Pool, builders: &[Box<dyn BuildExt>]) -> Result<BuildOutput>`

- [ ] **Step 1: Create `crates/oxibuilder-core/src/build.rs`**

```rust
use crate::builder::{BuildExt, BuildOutput, ExtBuildOutput};
use crate::db;
use rayon::prelude::*;

/// Run all extension builders in parallel via rayon.
/// Each extension produces pages, data, and search docs independently.
pub fn build_site(
    db: &db::Pool,
    builders: &[Box<dyn BuildExt>],
) -> Result<BuildOutput, Box<dyn std::error::Error>> {
    let results: Vec<ExtBuildOutput> = builders
        .par_iter()
        .map(|ext| {
            let pages = ext.build_pages(db)
                .map_err(|e| format!("[{}] build_pages: {}", ext.ext_id(), e))?;
            let data = ext.build_data(db)
                .map_err(|e| format!("[{}] build_data: {}", ext.ext_id(), e))?;
            let search_docs = ext.build_search_docs(db)
                .map_err(|e| format!("[{}] build_search_docs: {}", ext.ext_id(), e))?;
            Ok(ExtBuildOutput {
                ext_id: ext.ext_id().to_string(),
                pages,
                data,
                search_docs,
            })
        })
        .collect::<Result<Vec<_>, String>>()
        .map_err(|e| e)?;

    let mut output = BuildOutput {
        pages: Vec::new(),
        search_docs: Vec::new(),
        extensions_data: Vec::new(),
    };

    for r in results {
        output.pages.extend(r.pages);
        output.search_docs.extend(r.search_docs);
        output.extensions_data.push((r.ext_id, r.data));
    }

    Ok(output)
}
```

Note: `BuildExt` needs an `ext_id()` method. Add to the trait:

```rust
pub trait BuildExt: Send + Sync {
    type Error: Error + Send + 'static;
    fn ext_id(&self) -> &'static str;
    // ... rest unchanged
}
```

- [ ] **Step 2: Add `BuildExtRegistry` to registry module**

In `crates/oxibuilder-core/src/registry.rs`, add a method to collect `BuildExt` instances from all extensions. Each extension crate exports both its `Extension` impl and its `BuildExt` impl. The server binary (`oxibuilder-server/src/main.rs`) collects both.

Add:
```rust
/// Collect all BuildExt implementations from enabled extensions.
pub fn collect_builders(enabled: &[Box<dyn BuildExt>]) -> Vec<Box<dyn BuildExt>> {
    // This is called from the server binary with the concrete list.
    enabled.to_vec()
}
```

- [ ] **Step 3: Wire in `oxibuilder-server/src/main.rs`**

The server binary already links all extensions. Add a `collect_builders()` call alongside the existing `registry::register()`.

In `oxibuilder-server/src/main.rs`, after creating the extension registry, also collect BuildExt instances for use by the CLI build command.

- [ ] **Step 4: Verify compilation**

```bash
cargo check -p oxibuilder-core -p oxibuilder-server
```

- [ ] **Step 5: Git commit**

```
git add crates/oxibuilder-core/src/build.rs crates/oxibuilder-core/src/registry.rs
git commit -m "feat(core): add rayon parallel build pipeline"
```

---

### Task 3: BuildExt impl for profile extension

**Files:**
- Modify: `crates/oxibuilder-ext-profile/src/lib.rs`

**Interfaces:**
- Consumes: `BuildExt` trait from oxibuilder-core
- Produces: `impl BuildExt` for profile

**Context:** Profile is a singleton — one page at `/profile/`. The data file at `out/data/profile.json` contains the full profile object.

- [ ] **Step 1: Read current lib.rs**

```bash
cat crates/oxibuilder-ext-profile/src/lib.rs
```

- [ ] **Step 2: Implement BuildExt**

```rust
impl BuildExt for ProfileExt {
    type Error = anyhow::Error;

    fn ext_id(&self) -> &'static str { "profile" }

    fn build_pages(&self, db: &Pool) -> Result<Vec<StaticPage>> {
        let profile = repo::ProfileRepo::new(db).get()?;
        if profile.is_none() { return Ok(vec![]); }
        let profile = profile.unwrap();
        let html = render_profile_page(&profile); // simple HTML template
        Ok(vec![StaticPage {
            path: "profile/index.html".into(),
            content: html,
        }])
    }

    fn build_data(&self, db: &Pool) -> Result<Box<dyn erased_serde::Serialize + Send>> {
        let profile = repo::ProfileRepo::new(db).get()?;
        Ok(Box::new(profile))
    }

    fn build_search_docs(&self, db: &Pool) -> Result<Vec<SearchDoc>> {
        Ok(vec![]) // profile is not searchable
    }
}
```

- [ ] **Step 3: Add `anyhow::Error` as the error type**

Add `anyhow` dep if not present in extension's Cargo.toml. Most extensions already have it via workspace.

- [ ] **Step 4: Verify**

```bash
cargo check -p oxibuilder-ext-profile
```

- [ ] **Step 5: Git commit**

```
git add crates/oxibuilder-ext-profile/src/lib.rs
git commit -m "feat(profile): add BuildExt implementation"
```

---

### Task 4: BuildExt impl for blog extension

**Files:**
- Modify: `crates/oxibuilder-ext-blog/src/lib.rs`

**Interfaces:**
- Produces: `impl BuildExt` for blog

**Context:** Blog has published posts only (drafts excluded by `WHERE published_at IS NOT NULL`). Each post gets: `blog/{slug}/index.html` (prerendered HTML with OG metas + SPA script tag + `<div id="root">` for hydration), `blog/{slug}/index.md` (original markdown), `blog/{slug}/index.json` (metadata). Data file: `out/data/blog.json` (list of published posts with title/slug/published_at/tags).

- [ ] **Step 1: Implement `build_pages`**

Iterate published posts. For each: render HTML shell with `<title>`, `<meta property="og:...">`, markdown body converted to HTML (use existing markdown parser from routes), wrap in the standard SPA shell (`<div id="root">` + script tags to load React bundle). Also write `index.md` (copy body field).

HTML shell structure:
```html
<!DOCTYPE html>
<html lang="ko">
<head>
  <title>{title}</title>
  <meta property="og:title" content="{title}">
  <meta property="og:url" content="/blog/{slug}/">
  <meta property="og:type" content="article">
  <meta property="og:description" content="{excerpt}">
  <link rel="canonical" href="/blog/{slug}/">
</head>
<body>
  <div id="root"><!-- SSR content for SEO --><article>…</article></div>
  <script src="/assets/index.js"></script>
</body>
</html>
```

- [ ] **Step 2: Implement `build_data`**

Return a `Serialize` struct with all published posts.

- [ ] **Step 3: Implement `build_search_docs`**

One SearchDoc per published post.

- [ ] **Step 4: Verify**

```bash
cargo check -p oxibuilder-ext-blog
```

- [ ] **Step 5: Git commit**

```
git add crates/oxibuilder-ext-blog/src/lib.rs
git commit -m "feat(blog): add BuildExt with markdown source output"
```

---

### Task 5: BuildExt impl for projects extension

**Files:**
- Modify: `crates/oxibuilder-ext-projects/src/lib.rs`

**Context:** Projects with screenshots. `build_pages` generates `projects/{slug}/index.html` (prerendered with tech stack, description, screenshot gallery). `build_data` returns the project list with full details. `build_search_docs` indexes title + description.

- [ ] **Step 1: Read current lib.rs to understand repo types**

- [ ] **Step 2: Implement `build_pages`**

Each published/featured project gets an HTML page. Gallery screenshots are referenced as relative paths (`/media/projects/{slug}/screenshots/...`).

- [ ] **Step 3: Implement `build_data` and `build_search_docs`**

- [ ] **Step 4: Verify compilation**

- [ ] **Step 5: Git commit**

```
git commit -m "feat(projects): add BuildExt implementation"
```

---

### Task 6: BuildExt impl for remaining 6 extensions (novels, movies, books, scraps, links, activity)

**Files:**
- Modify: `crates/oxibuilder-ext-novels/src/lib.rs`
- Modify: `crates/oxibuilder-ext-movies/src/lib.rs`
- Modify: `crates/oxibuilder-ext-books/src/lib.rs`
- Modify: `crates/oxibuilder-ext-scraps/src/lib.rs`
- Modify: `crates/oxibuilder-ext-links/src/lib.rs`
- Modify: `crates/oxibuilder-ext-activity/src/lib.rs`

**Context:** Same pattern as Tasks 3-5. Each extension:
- `build_pages`: published content → HTML with OG tags
- `build_data`: list/summary → JSON for client-side fetch
- `build_search_docs`: SearchDoc per published item
- For `novels`: each chapter is a separate HTML page at `novels/{slug}/chapter-{n}/index.html`. Also output `index.md` for chapters.
- For `activity`: no published_at concept, build the full timeline as static JSON. Activity is read-only cache.
- For `links`: featured + non-featured links in list page.

**Execution note:** All 6 extensions are identical in pattern. A subagent can batch these.

- [ ] **Step 1-5: novels BuildExt**
- [ ] **Step 6-10: movies BuildExt**
- [ ] **Step 11-15: books BuildExt**
- [ ] **Step 16-20: scraps BuildExt**
- [ ] **Step 21-25: links BuildExt**
- [ ] **Step 26-30: activity BuildExt**

---

### Task 7: Wire all BuildExt impls into server binary

**Files:**
- Modify: `crates/oxibuilder-server/src/main.rs`

**Context:** The server binary statically links all extensions. It already has a list of all extension instances. Add a function that returns all BuildExt instances from the same list.

- [ ] **Step 1: Find where extensions are registered**

Read `crates/oxibuilder-server/src/main.rs` to see the registration pattern.

- [ ] **Step 2: Add `builders()` function**

```rust
pub fn builders() -> Vec<Box<dyn BuildExt>> {
    vec![
        Box::new(ProfileExt),
        Box::new(BlogExt),
        Box::new(ProjectsExt),
        // ... all 9
    ]
}
```

- [ ] **Step 3: Export this list for CLI use**

The CLI doesn't import the server binary. Instead, create a new shared crate or expose through oxibuilder-core. Better approach: the CLI talks to the server's API for build, and the server has a `/api/v1/build` endpoint that triggers the build pipeline.

Actually, rethinking: the build pipeline should run locally against the local DB. The CLI launches it via `oxibuilder build` which calls into `oxibuilder-core`'s build module.

Simpler: create `crates/oxibuilder-build/` that links all extensions and exposes a single `fn build_all(db: &Pool) -> Result<BuildOutput>`.

Or even simpler: the CLI binary itself links all extensions (it already links them via `oxibuilder-cli` depending on the extensions).

Let me check: does `oxibuilder-cli` depend on each extension crate?

```bash
grep -r "oxibuilder-ext" crates/oxibuilder-cli/Cargo.toml
```

If yes, the CLI can run build logic directly. If not, we need a shared binary or a build sub-crate.

- [ ] **Step 3: Add build endpoint to server**

Add `POST /api/v1/build` endpoint to the server. This is simpler than making the CLI link all extensions. The server already has all extensions.

```rust
async fn handle_build(State(state): State<AppState>) -> Json<BuildResult> {
    // run build pipeline
    let output = build_site(&state.db, &state.builders)?;
    // write to out/
    write_output(&output, &state.config.site.out_dir)?;
    Ok(Json(BuildResult { success: true }))
}
```

- [ ] **Step 4: Git commit**

```
git commit -m "feat(server): wire BuildExt registry and /api/v1/build endpoint"
```

---

### Task 8: `oxibuilder build` CLI command

**Files:**
- Create: `crates/oxibuilder-cli/src/commands/build.rs`
- Modify: `crates/oxibuilder-cli/src/commands/mod.rs`

- [ ] **Step 1: Create build command**

```rust
#[derive(clap::Args)]
pub struct BuildCommand {
    /// Optional: only build specific site (from sites.toml)
    #[arg(long)]
    site: Option<String>,
}

pub async fn run(args: BuildCommand, cli: &Cli) -> anyhow::Result<()> {
    // 1. Resolve server endpoint
    // 2. POST /api/v1/build
    // 3. Wait for completion
    // 4. Report results
}
```

- [ ] **Step 2: Register in dispatch**

Add `BuildCommand` to the clap enum and dispatch table in `mod.rs`.

- [ ] **Step 3: Test**

```bash
cargo check -p oxibuilder-cli
```

- [ ] **Step 4: Git commit**

---

### Task 9: `oxibuilder deploy` CLI command

**Files:**
- Create: `crates/oxibuilder-cli/src/commands/deploy.rs`
- Modify: `crates/oxibuilder-cli/src/commands/mod.rs`

**Context:** Deploy `out/` to target platform. First target: GitHub Pages.

- [ ] **Step 1: Create deploy command**

```rust
#[derive(clap::Args)]
pub struct DeployCommand {
    #[arg(long, default_value = "github-pages")]
    target: String,
    #[arg(long)]
    site: Option<String>,
}

pub async fn run(args: DeployCommand, cli: &Cli) -> anyhow::Result<()> {
    match args.target.as_str() {
        "github-pages" => deploy_github_pages(&cli).await?,
        "cloudflare" => deploy_cloudflare(&cli).await?,
        _ => anyhow::bail!("unsupported target: {}", args.target),
    }
    Ok(())
}
```

- [ ] **Step 2: Implement GitHub Pages deploy**

```rust
async fn deploy_github_pages(cli: &Cli) -> anyhow::Result<()> {
    // 1. Check gh CLI is installed: cmd("gh --version")
    // 2. gh auth status
    // 3. git worktree add /tmp/oxibuilder-deploy gh-pages (or create orphan branch)
    // 4. cp -rf out/* /tmp/oxibuilder-deploy/
    // 5. cd /tmp/oxibuilder-deploy && git add -A && git commit -m "deploy: ..."
    // 6. git push origin gh-pages
    // 7. git worktree remove /tmp/oxibuilder-deploy
}
```

Use `std::process::Command` for git operations (not git2 library — simpler).

- [ ] **Step 3: Register in dispatch**

- [ ] **Step 4: Git commit**

---

### Task 10: `oxibuilder query` and `oxibuilder schema` CLI commands

**Files:**
- Create: `crates/oxibuilder-cli/src/commands/query.rs`
- Create: `crates/oxibuilder-cli/src/commands/schema.rs`
- Modify: `crates/oxibuilder-cli/src/commands/mod.rs`

**Context:** AI agent friendly — direct SQLite access from CLI. Read-only queries.

- [ ] **Step 1: Create query command**

```rust
#[derive(clap::Args)]
pub struct QueryCommand {
    #[arg(required = true)]
    sql: String,
    #[arg(long)]
    json: bool,
}

pub async fn run(args: QueryCommand, cli: &Cli) -> anyhow::Result<()> {
    // 1. Resolve DB path from config (oxibuilder.toml or default data_dir)
    let db_path = resolve_db_path(&cli)?;
    // 2. Open SQLite connection (sqlx::SqliteConnection::connect)
    // 3. Execute read-only query
    // 4. Output as JSON or table
}
```

**Key constraint:** Only allow SELECT statements. Block INSERT/UPDATE/DELETE/DROP:

```rust
fn is_read_only(sql: &str) -> bool {
    let trimmed = sql.trim().to_uppercase();
    trimmed.starts_with("SELECT") || trimmed.starts_with("PRAGMA") || trimmed.starts_with("EXPLAIN")
}
```

- [ ] **Step 2: Create schema command**

```rust
#[derive(clap::Args)]
pub struct SchemaCommand {
    #[arg(long)]
    extension: Option<String>,
    #[arg(long)]
    json: bool,
}

pub async fn run(args: SchemaCommand, cli: &Cli) -> anyhow::Result<()> {
    // 1. Open DB
    // 2. Query sqlite_master for tables, columns, types
    // 3. Filter by extension name pattern (ext_% prefix convention)
    // 4. Output as JSON or formatted table
}
```

SQL:
```sql
SELECT m.name as table_name, p.name as column_name, p.type as column_type, p.pk
FROM sqlite_master m
JOIN pragma_table_info(m.name) p
WHERE m.type = 'table'
  AND m.name NOT LIKE 'sqlite_%'
  AND (?1 IS NULL OR m.name LIKE ?1)
ORDER BY m.name, p.cid
```

- [ ] **Step 3: Register both in dispatch**

- [ ] **Step 4: Verify**

```bash
cargo check -p oxibuilder-cli
```

- [ ] **Step 5: Git commit**

```
git commit -m "feat(cli): add query and schema commands for AI agent SQL access"
```

---

### Task 11: `oxibuilder cache refresh` CLI command

**Files:**
- Create: `crates/oxibuilder-cli/src/commands/cache.rs`
- Modify: `crates/oxibuilder-cli/src/commands/mod.rs`

**Context:** Move external API calls (GitHub, TMDB, Aladin, HN) from background jobs to explicit command.

- [ ] **Step 1: Create cache command**

```rust
#[derive(clap::Args)]
pub struct CacheCommand {
    #[arg(long)]
    extension: Option<String>,
}

pub async fn run(args: CacheCommand, cli: &Cli) -> anyhow::Result<()> {
    // POST /api/v1/cache/refresh?extension=... to the server
    // The server runs the background job logic synchronously
}
```

- [ ] **Step 2: Add server endpoint**

In oxibuilder-core HTTP routes: `POST /api/v1/cache/refresh` that triggers the existing scheduler logic once.

```rust
async fn handle_cache_refresh(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Value>, AppError> {
    let ext_id = params.get("extension").map(String::as_str);
    // run the scheduler job handlers synchronously for requested extension
    // (reuse existing job logic)
}
```

- [ ] **Step 3: Git commit**

---

### Task 12: `oxibuilder serve --preview` mode

**Files:**
- Modify: `crates/oxibuilder-cli/src/commands/init_status_serve.rs`
- Modify: `crates/oxibuilder-core/src/http.rs`

**Context:** Serve `out/` directory as static files for local preview.

- [ ] **Step 1: Add --preview flag to serve command**

```rust
pub struct ServeCommand {
    #[arg(long)]
    pub port: Option<u16>,
    #[arg(long)]
    pub preview: bool,
}
```

- [ ] **Step 2: Implement preview mode**

When `--preview` is set, start a lightweight HTTP server that serves `out/` directory. Use `tower-http::services::ServeDir` from existing deps.

```rust
async fn run_preview(config: &Config) -> anyhow::Result<()> {
    let out_dir = config.site.out_dir();
    let app = Router::new()
        .nest_service("/", ServeDir::new(&out_dir));
    let listener = TcpListener::bind(("127.0.0.1", port)).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
```

- [ ] **Step 3: Git commit**

---

### Task 13: React SPA data layer switch (VITE_DATA_MODE)

**Files:**
- Modify: `web/src/shared/api.ts` (or wherever TanStack Query fetchers live)
- Modify: `web/.env` (add VITE_DATA_MODE)

**Context:** The SPA currently fetches from `/api/v1/...`. In static mode, it fetches from `/data/...json`.

- [ ] **Step 1: Find current API fetch pattern**

```bash
grep -r "fetch(" web/src/ | head -20
grep -r "api/v1" web/src/ | head -20
```

- [ ] **Step 2: Create API url resolver**

```typescript
// web/src/shared/api.ts
const DATA_MODE = import.meta.env.VITE_DATA_MODE || 'api';

export function apiUrl(path: string): string {
  if (DATA_MODE === 'static') {
    // Map /api/v1/blog/posts → /data/blog.json
    return `/data/${path.split('/')[3]}.json`;
  }
  return path; // original behavior
}
```

- [ ] **Step 3: Update all fetcher functions**

Wrap existing TanStack Query `queryFn` calls with `apiUrl()`.

- [ ] **Step 4: Update .env files**

```
VITE_DATA_MODE=api     # development (default)
VITE_DATA_MODE=static  # production / preview
```

- [ ] **Step 5: Verify with bun dev**

```bash
cd web && bun run build
```

- [ ] **Step 6: Git commit**

---

### Task 14: File writer for build output (out/ directory)

**Files:**
- Create: `crates/oxibuilder-core/src/build_writer.rs`

**Context:** Write the `BuildOutput` to the `out/` directory as static files.

- [ ] **Step 1: Create writer module**

```rust
pub fn write_build_output(output: &BuildOutput, out_dir: &Path) -> Result<()> {
    // 1. Clean out_dir (or backup)
    // 2. Write all StaticPage files
    for page in &output.pages {
        let path = out_dir.join(&page.path);
        fs::create_dir_all(path.parent().unwrap())?;
        fs::write(&path, &page.content)?;
    }
    // 3. Write extension data JSON files
    for (ext_id, data) in &output.extensions_data {
        let json = serde_json::to_string_pretty(data)?;
        let path = out_dir.join("data").join(format!("{ext_id}.json"));
        fs::create_dir_all(path.parent().unwrap())?;
        fs::write(&path, &json)?;
    }
    // 4. Write search index
    let search_json = serde_json::to_string_pretty(&output.search_docs)?;
    fs::write(out_dir.join("data").join("search-index.json"), &search_json)?;
    // 5. Copy web/dist/ assets
    copy_dir(web_dist, out_dir.join("assets"))?;
    // 6. Copy media files
    copy_dir(media_dir, out_dir.join("media"))?;
    Ok(())
}
```

- [ ] **Step 2: Add out_dir to config**

`oxibuilder.toml` already has `[server] data_dir`. Add `out_dir` (default: `data/out`).

- [ ] **Step 3: Wire into build server endpoint**

The `/api/v1/build` endpoint calls both `build_site()` and `write_build_output()`.

- [ ] **Step 4: Git commit**

---

### Task 15: Verify full pipeline end-to-end

**Context:** Run the complete flow: content exists in DB → `oxibuilder build` → `out/` generated → `oxibuilder deploy` → GitHub Pages.

- [ ] **Step 1: Manual integration test**

```bash
# 1. Start server
./target/debug/oxibuilder-server &
# 2. Create a blog post
oxibuilder blog new "Test SSG" --lang ko --file /tmp/test.md --json
oxibuilder blog publish test-ssg
# 3. Build
oxibuilder build
# 4. Check output
ls -la out/
ls -la out/blog/test-ssg/
# 5. Preview
oxibuilder serve --preview &
open http://127.0.0.1:8787
```

- [ ] **Step 2: Run test suite**

```bash
cargo test --workspace
cargo clippy --all-targets -- -D warnings
cd web && bun run build
```

- [ ] **Step 3: Deploy test**

```bash
oxibuilder deploy --target github-pages --dry-run
```

- [ ] **Step 4: Git commit any fixes**

---

### Task 16: Update oxibuilder-cli SKILL.md for new commands

**Files:**
- Modify: `.agent/skills/oxibuilder-cli/SKILL.md`

**Context:** The AI agent skill doc needs to document the new commands (build, deploy, query, schema, cache).

- [ ] **Step 1: Add new commands to the skill doc**

- [ ] **Step 2: Update the workflow examples**

- [ ] **Step 3: Git commit**

---

### Task 17: Clean up — remove unused v1 deployment artifacts

**Files:**
- Consider: `deploy/oxibuilder.plist.example` (no longer needed for public site)
- Consider: `deploy/oxibuilder.service.example`
- Consider: `deploy/Caddyfile.example`
- Keep: `deploy/Dockerfile` (still useful for local management server)

**Context:** Clean up outdated deployment templates that reference the old self-hosted server model.

- [ ] **Step 1: Mark deprecated deployment files with a note**

Add a deprecation notice at the top of each file rather than deleting them — the user may still use them for the local management server.

- [ ] **Step 2: Git commit**

---

## Self-Review against spec

- **BuildExt trait**: Task 1 ✅
- **rayon parallel build**: Task 2 ✅
- **9 extension BuildExt impls**: Tasks 3-6 ✅
- **build CLI command**: Task 8 ✅ (via server API)
- **deploy CLI command**: Task 9 ✅
- **query/schema CLI**: Task 10 ✅
- **cache refresh**: Task 11 ✅
- **serve --preview**: Task 12 ✅
- **React VITE_DATA_MODE**: Task 13 ✅
- **out/ file writer**: Task 14 ✅
- **markdown source output**: Task 4 (blog) + Task 6 (novels) ✅
- **search index as JSON**: Task 2 (build pipeline) ✅
- **media copy**: Task 14 ✅
- **agent skill update**: Task 16 ✅
- **cleanup deprecated deploy artifacts**: Task 17 ✅
