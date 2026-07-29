# Oxipage → Production: Design & Implementation Plan

> Companion to `docs/production-readiness-report.md`. 2026-07-29.
> Scope: turn the documented `cargo install → oxipage build → oxipage deploy` path into something
> that produces a working static site. Phased so each phase is independently shippable.

## 1. Goal & non-goals

**Goal:** `cargo install oxipage && oxipage build && oxipage deploy --target github-pages` deploys a
public site whose lobby, collection, and **detail** pages all render from static JSON, search works,
and the SPA bundle is present.

**Non-goals (explicitly deferred):** re-adding authentication (loopback-only is the design),
Cloudflare/Netlify deploy (out of P0 scope), multi-tenant, frontend test framework beyond a build
smoke. These are tracked as P2/P3 in the report.

## 2. P0-1 fix — build pipeline: pass the runtime handle into `BuildExt`

**Problem:** `build_site` runs `BuildExt` on rayon threads; each impl calls
`Handle::current()` (panics — no runtime on a rayon thread).

**Design:** `Handle::block_on` is safe to call from a non-runtime thread **if you already hold a
valid `Handle`**. Capture the handle once on the runtime thread (the `async fn build` entry point),
before rayon, and pass `&Handle` into every `BuildExt` method.

```rust
// oxipage-core/src/builder.rs — trait gains a handle parameter
pub trait BuildExt: Send + Sync {
    type Error: std::error::Error + Send + 'static;
    fn ext_id(&self) -> &'static str;
    fn build_pages(&self, db: &SqlitePool, rt: &tokio::runtime::Handle)
        -> Result<Vec<StaticPage>, Self::Error>;
    fn build_data(&self, db: &SqlitePool, rt: &tokio::runtime::Handle)
        -> Result<Box<dyn erased_serde::Serialize + Send>, Self::Error>;
    fn build_search_docs(&self, db: &SqlitePool, rt: &tokio::runtime::Handle)
        -> Result<Vec<SearchDoc>, Self::Error>;
}

// oxipage-core/src/build.rs — capture once, before rayon
pub fn build_site(db, builders) -> Result<BuildOutput> {
    let rt = tokio::runtime::Handle::current();   // on the runtime thread ✅
    builders.par_iter().map(|ext| {
        let pages = ext.build_pages(db, &rt)?;     // block_on runs future on runtime ✅
        ...
    }).collect()
}
```

Each of the 9 impls drops its `let handle = Handle::current();` line and uses the `rt` param. This is
mechanical and preserves the rayon parallelism + the existing `block_on` bodies.

**Why not make `BuildExt` async?** It would be cleaner (the work is DB-I/O, not CPU-bound — the
design spec's "CPU-bound, use rayon" premise is wrong), but it is a larger conceptual change and
forces `futures`-based concurrency in `build_site`. The handle parameter is the minimal correct fix;
async-ification is a later refactor (P3).

## 3. P0-2 fix — build writes `out/assets/` from the embedded binary, not CWD

**Problem:** `cli build.rs:25` reads `web/dist` from the working directory. Outside the repo it's
absent → empty assets.

**Design:** the binary already embeds the SPA via `rust-embed` (`http.rs::Assets`,
`#[folder = "embedded-spa"]`). Expose an iterator over embedded assets and write them to
`out/assets/`. `build_writer` gains an `assets: impl Iterator<Item = (String, Vec<u8>)>` (or takes the
`Assets` type) instead of a `web_dist: &Path`. The `web_dist` filesystem path is dropped entirely.

```rust
// build_writer.rs — replace web_dist: &Path with embedded source
pub fn write_build_output(output, out_dir, media_dir, embedded_assets: &Assets) -> Result<()> {
    ...
    for (path, bytes) in embedded_assets.iter() {
        write(out_dir.join("assets").join(path), bytes);
    }
}
```

`oxipage_core::http::Assets` already supports `iter()`/`get()` via `rust-embed`. Add a thin
`pub fn embedded_files() -> Vec<(String, Vec<u8>)>` helper.

## 4. P0-3 fix — ship the SPA in the published crate

**Problem:** `embedded-spa/` + `web/dist` are gitignored; no `Cargo.toml include`; `cargo publish`
excludes them → `cargo install` gets a placeholder.

**Design (two-part, both needed):**

1. **Source the SPA into the package at publish time.** Add `include` to
   `crates/oxipage-core/Cargo.toml` (and `oxipage-console`) so the built `embedded-spa/` is packaged:
   ```toml
   include = ["src/**", "migrations/**", "embedded-spa/**", "_registry.json", "_wasm-demo.wasm", "build.rs", "Cargo.toml"]
   ```
   Then un-gitignore the *generated* `embedded-spa/` (it is a build artifact, not source). Two viable
   policies — **recommended: build-in-CI**: keep `embedded-spa/` gitignored as a local artifact, but
   have `release.yml` run `bun run build` for `web/` and `admin-web/`, then `cargo publish` with the
   populated `embedded-spa/` already on disk (build.rs copies `web/dist`→`embedded-spa` before
   packaging). Add a CI assertion that `cargo package --list` contains `embedded-spa/index.html`.

2. **Ship prebuilt release binaries** (deferred, P2): `release.yml` cross-builds `oxipage` +
   `oxipage-console` and attaches them to the GitHub release. This is the path most users want
   (avoids a Rust toolchain entirely). Tracked, not in the P0 cut.

**Decision recorded for the report:** P0 cut = option (1) build-in-CI + `include` + CI assertion.
Prebuilt binaries = P2 follow-up.

## 5. P0-4 / P0-5 fix — SPA static data contract

**Problem:** `pathToStaticFile` collapses every URL to a single collection JSON; detail pages and
search receive the wrong shape.

**Design — backend (`build_data`):** keep collection JSON (`data/blog.json` = list) **and** add
per-item JSON that includes the body:
```
data/blog.json              → BlogPost[]   (list, no heavy body — keep lobby/list fast)
data/blog/<slug>.json       → BlogPost     (full, with body)
data/search-index.json      → SearchDoc[]  (unchanged)
```
`build_writer` already writes arbitrary `pages`; extend `build_data` (or `build_pages`) to also emit
`data/<ext>/<key>.json` for detail-able content (blog, projects, novels/chapters, movies, books).
Profile/links/scraps/activity stay collection-only (no detail route).

**Design — frontend (`api.ts`):**
- `pathToStaticFile('/blog/<slug>')` → `/data/blog/<slug>.json` (not `/data/blog.json`). Detect
  detail routes by a 2-segment path.
- `searchAll(q)` in static mode: `fetch('/data/search-index.json')` once, then filter client-side by
  `title.includes(q) || body_preview.includes(q)` (design spec §5.2). Cache the index in module scope.

This keeps the live/REST mode untouched (`VITE_DATA_MODE !== 'static'` path is unchanged).

## 6. P1-1 fix — flatten the `build` command

**Problem:** `Build(BuildCommand)` + `#[command(subcommand)]` forces `oxipage build build`.

**Design:** change `BuildCommand` from an `enum { Run { out_dir } }` to a `struct { out_dir }`
with `#[derive(Args)]`. `Command::Build(BuildCommand)` then flattens → `oxipage build [--out-dir X]`.
Update the `build()` dispatch arm to read `c.out_dir` directly.

## 7. P1-2 / docs — deploy targets & doc-vs-code reconciliation

- Either implement cloudflare/netlify (wrangler/netlify CLI shell-out, mirroring the github-pages
  flow) **or** correct README:14/207 to "GitHub Pages (Cloudflare/Netlify planned)". P0 cut: correct
  the docs (don't advertise what doesn't exist).
- Fix README binary name `oxipage-server` → `oxipage-console` (install + usage).
- Fix README:31 + `doc/06` Phase 6 status to "implemented".
- Rewrite `.agent/skills/oxipage-cli/SKILL.md` and `docs/extension-sdk.md` to drop `auth`/`AdminAuth`/
  `oxipage-server`.

## 8. Hardening roadmap (P2, after the P0 cut)

| ID | Item | Sketch |
|---|---|---|
| P2-1 | Dead auth plumbing | drop `bearer_auth` send in `client.rs` + the `_admin_token` test fixtures, or wire a feature-gated token middleware |
| P2-2 | Remote-site framing | document that remote consoles need a reverse-proxy auth layer; don't imply the token protects anything |
| P2-3 | `busy_timeout` | `PRAGMA busy_timeout = 5000` in `db.rs` connect |
| P2-4 | deploy rewrite | `std::fs` copy + `scopeguard`/`Drop` worktree cleanup; drop `bash -c` |
| P2-5 | ops artifacts | restore `deploy/Dockerfile` (non-root), Caddyfile, plist, systemd |
| P2-6 | rate-limit IP | trusted-proxy hop config or `ConnectInfo`-based |
| P2-7 | scheduler isolation | `tokio::time::timeout` + `catch_unwind` per job |
| P2-8 | dep audit | scheduled `cargo audit` in CI |

## 9. Test strategy (prevent P0 regression)

The build panic shipped because **nothing exercised `build_site`**. Add:

1. **`oxipage-core` build integration test:** seed a temp DB with a published blog post, run
   `build_site` (with the captured handle), assert `output.pages` contains `blog/<slug>/index.html`
   and `output.search_docs` is non-empty. This would have caught P0-1.
2. **`build_writer` test:** write to a temp dir, assert `out/data/blog.json`,
   `out/data/blog/<slug>.json`, `out/assets/index.html` (from embedded) exist.
3. **CLI e2e:** `oxipage build` (flattened) against a temp DB → assert `out/` non-empty and a known
   file present. Runs in the existing `crates/oxipage-cli/tests/e2e.rs`.

Frontend unit tests (P3) deferred.

## 10. Phased rollout

- **Phase A (P0 cut, this branch):** §2 (runtime handle), §3 (embedded assets), §5 (SPA data
  contract), §6 (flatten build), §9 (tests). Outcome: `oxipage build` works repo-locally and produces
  a correct static site.
- **Phase B (distribution):** §4 (build-in-CI + `include` + assertion). Outcome: `cargo install`
  works.
- **Phase C (docs/deploy):** §7. Outcome: no advertised feature is a lie.
- **Phase D (hardening):** §8, incrementally.

Branch `production-readiness` carries Phase A (the code changes); Phase B–C are CI/docs edits that
can land together.
