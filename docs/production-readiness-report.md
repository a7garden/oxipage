# Oxibuilder Production-Readiness Report

> Assessment date: 2026-07-29. Baseline: `v0.4.0` on `main` (commit `d0314d7`).
> Method: read all 14 crates, 4 parallel read-only audits, plus **empirical execution** of the
> build/CLI/test/packaging gates. Severity is load-bearing: P0 = the core product does not function;
> P1 = breaks a documented workflow; P2 = hardening; P3 = polish.

## 0. Executive summary

Oxibuilder's surface is broad and mostly implemented: 9 extensions, two React SPAs (`web/` public +
`admin-web/` management), a working local management server, backup, search, multi-site profiles,
WASM runtime, and a release pipeline that publishes 13 crates to crates.io. The codebase is clean on
clippy and **137 tests pass (3 ignored)**.

**It is not production-ready.** Three independent P0 failures, each verified by *running* it rather
than reading it, span the entire SSG pipeline from build → asset embedding → distribution:

1. **`oxibuilder build` panics on every extension** (no Tokio runtime on rayon threads).
2. **`oxibuilder build` reads the SPA from CWD `web/dist`, not the embedded binary** → blank pages when
   run outside the repo checkout.
3. **`cargo install oxibuilder` ships a placeholder SPA.** `embedded-spa/` and `web/dist` are gitignored
   and not in any `Cargo.toml include`, so the published crate contains no frontend at all.

Combined: even if the build panic were fixed, a user who `cargo install`s oxibuilder and runs
`oxibuilder build && oxibuilder deploy` gets a deployed site with **no working pages and no JS bundle**.
On top of that, the public SPA's static data layer cannot render any **detail page** (blog post,
project) or **search** in static mode.

The deliverable that matters: make the documented `cargo install → oxibuilder build → oxibuilder deploy`
path produce a working static site end to end. Everything below is secondary.

## 1. What actually works (verified)

| Area | Status | Evidence |
|---|---|---|
| Management server (`oxibuilder console`) | ✅ Works | boots, serves embedded admin-web + `/api/console/*`, SQLite WAL, graceful SIGTERM/SIGINT |
| CLI content CRUD (blog/project/link) | ✅ Works | direct SQLite repo access, no HTTP round-trip |
| 9 extensions' manage-side (routes, CLI, jobs) | ✅ Works | 137 tests pass, clippy clean |
| Webhook HMAC-SHA256 (GitHub activity) | ✅ Correct | `oxibuilder-ext-activity/src/routes.rs:50-113`, constant-time compare, 503 on missing secret |
| Setup-wizard loopback gate | ✅ Correct | `setup.rs:226-259` rejects non-loopback /setup/* with 403 |
| Secret indirection (env-var names in config) | ✅ Correct | `config.rs:83-98` stores names only |
| Multi-site profiles (`sites.toml`, 0600) | ✅ Works | CLI `site` CRUD + 3-tier endpoint resolution |
| Backup (`VACUUM INTO` snapshot) | ✅ Works | `backup.rs` + CLI `backup snapshot` |
| Release CI | ✅ Works | tiered `cargo publish` + GitHub release draft on `v*` tag |
| Public SPA in **live/REST** mode | ✅ Works | `web/src/shared/api.ts` `/api/console/*` path |
| Repo-local build with `web/dist` present | ✅ Embeds | `oxibuilder-core/build.rs:8-11` copies `web/dist`→`embedded-spa`, `rust-embed` bakes it in |

## 2. P0 — Critical (core product broken)

### P0-1. `oxibuilder build` panics; SSG produces no output
- **Evidence (empirical):** `./target/release/oxibuilder build build` → 5 panics across extensions:
  `oxibuilder-ext-blog/src/lib.rs:119`, `…movies…:207`, `…scraps…:324`, `…activity…:178`,
  `…links…:81` — `"there is no reactor running, must be called from the context of a Tokio 1.x runtime"`.
  Output directory `data/out/` left empty.
- **Root cause:** `build_site` (`oxibuilder-core/src/build.rs:21`) drives builders with
  `rayon::par_iter()`. Every builder's `build_pages`/`build_data`/`build_search_docs` begins with
  `let handle = tokio::runtime::Handle::current();` then `handle.block_on(repo::…)`. Rayon worker
  threads are not Tokio-runtime threads, so `Handle::current()` panics. All 9 extensions are affected
  (49 `Handle::current()`/`block_on` sites across `crates/oxibuilder-ext-*/src/lib.rs`).
- **Why it shipped:** no test calls `build_site` / `BuildExt`. The SSG has zero output verification.
- **Fix design:** see `docs/production-design.md` §2.

### P0-2. `oxibuilder build` copies the SPA from CWD `web/dist`, not the embedded binary
- **Evidence:** `oxibuilder-cli/src/commands/build.rs:25` sets `let web_dist = PathBuf::from("web/dist")`,
  passed to `build_writer::write_build_output` (`build_writer.rs:57` `if web_dist.exists()`).
  The binary already carries the SPA via `rust-embed` (`http.rs:22-23` `#[folder="embedded-spa"]`,
  served by the management server) — but the build command ignores it and reads the filesystem.
  Run the release binary from any directory other than the repo root and `out/assets/` is empty → the
  deployed site loads `<script src="/assets/index.js">` that 404s → blank pages.
- **Fix design:** `docs/production-design.md` §3 — write `out/assets/` from the embedded `Assets`,
  same source the management server uses.

### P0-3. `cargo install oxibuilder` ships a placeholder SPA (distribution broken)
- **Evidence (empirical):** `cargo package --list -p oxibuilder-core --no-verify` → 33 files, **zero**
  `embedded-spa/`, zero `web/dist`, zero `index.html`. `.gitignore:6` ignores `/web/dist`, `:8`
  `/admin-web/dist`, `:25` `embedded-spa/`. `git ls-files 'crates/*/embedded-spa/*'` → 0 tracked
  files. No `Cargo.toml` has an `include` key. `cargo publish` excludes gitignored files unless an
  `include` forces them — so the published 0.4.0 crates contain no frontend.
- **Consequence:** a `cargo install oxibuilder` user compiles `oxibuilder-core/build.rs` with no
  `../../web/dist` → placeholder branch (`build.rs:12-22`, "SPA not embedded") → the embedded binary
  serves a stub, and P0-2's `oxibuilder build` has nothing to copy. Management UI *and* build output are
  both broken for anyone who didn't `git clone` + `bun run build`.
- **Why it shipped:** `release.yml` only runs `cargo publish`; it uploads no prebuilt binaries to the
  GitHub release and CI does not assert that the packaged crate contains the SPA.
- **Fix design:** `docs/production-design.md` §4 — commit the built SPA (or generate + `include` it
  in CI before publish) and/or ship prebuilt release binaries.

### P0-4. Public SPA cannot render detail pages in static mode
- **Evidence:** `web/src/shared/api.ts:91-99` `pathToStaticFile()` maps `/blog/<slug>` →
  `/data/blog.json` (the whole collection). `BlogPostPage.tsx:17` calls `fetchBlogPost(slug)`
  typed as a single `BlogPost` but receives an array → `post.title` undefined, `post.tags.length`
  throws → the page crashes. Same defect for project detail.
- **Per-item data exists but is unused:** `build_pages` emits `blog/<slug>/index.json` containing
  metadata *only* (no `body`), and the SPA never fetches it. There is **no JSON path that carries a
  post's body** in static mode.
- **Fix design:** `docs/production-design.md` §5 — emit per-item JSON (with `body`) and make the SPA
  fetch the right file.

### P0-5. Public SPA search is a no-op in static mode
- **Evidence:** `searchAll(q)` → static mode → `pathToStaticFile('/search?q=…')` →
  `/data/search-index.json` (the entire index) returned as "results" with no filtering on `q`.
- **Fix design:** `docs/production-design.md` §5 — client-side substring filter over the loaded index
  (design spec §5.2 prescribes exactly this; never implemented).

## 3. P1 — Breaks a documented workflow

### P1-1. `oxibuilder build` requires `oxibuilder build build`
- **Evidence:** `main.rs:85-86` declares `#[command(subcommand)] Build(BuildCommand)` where
  `BuildCommand` is an `enum { Run { #[command(name="build")] } }`. The real command is
  `oxibuilder build build`. `Deploy` (`main.rs:90`) and `Cache` are flat. README:124, design §3.3, and
  SKILL.md all say `oxibuilder build`.
- **Fix:** flatten `BuildCommand` from enum to `Args` struct → `oxibuilder build [--out-dir X]`.

### P1-2. Deploy targets Cloudflare Pages / Netlify are advertised but unimplemented
- **Evidence:** README:14, :207 list GitHub Pages / Cloudflare / Netlify.
  `deploy.rs:25-27` bails `"cloudflare" | "netlify" => "not yet implemented"`.
- **Fix:** implement or correct the docs. See design §6.

### P1-3. README references a binary that does not exist
- **Evidence:** README:53, :56, :88 say `oxibuilder-server`. Merged into `oxibuilder-console`
  (`crates/oxibuilder-console/Cargo.toml`). Install/usage instructions are wrong.

### P1-4. Stale Status framing
- **Evidence:** README:31 "v2 SSG in design" and `doc/06` Phase 6 = "⏳ planning complete". The SSG is
  implemented (broken — P0-1). The roadmap and the README's own install steps disagree.

### P1-5. Stale agent/SDK docs
- **Evidence:** `.agent/skills/oxibuilder-cli/SKILL.md` references removed `auth` subcommands.
  `docs/extension-sdk.md` references the removed `AdminAuth` middleware and `oxibuilder-server`.

## 4. P2 — Hardening (security & operations)

Auth removal (commit `90b0140`) is **intentional and documented** — the management server is
loopback-only by design (README:97-102, 176-184). "Re-add auth" is **not** a gap; exposing the server
is the operator's reverse-proxy responsibility. The real hardening items:

- **P2-1. Dead auth code is misleading.** `client.rs:64-65` sends `bearer_auth(t)` on every request;
  the server has no middleware that reads it. Every extension test sends `Authorization: bearer tok`
  on write endpoints (`oxibuilder-ext-{blog,books,links}/tests/api.rs`, `http_app.rs:163`) — they pass
  only because nothing validates. Either drop the token plumbing or wire a real gate behind a flag.
- **P2-2. `sites.toml` remote tokens advertise false security.** `sites.rs` stores `endpoint + token`
  for remote consoles; the token is sent but never verified remotely. The "remote management" framing
  implies remote servers are safe to expose — they are not. Resolve the inconsistency: drop the
  remote-management framing, or document that remote consoles require a reverse-proxy auth layer.
- **P2-3. SQLite `busy_timeout` is unset.** `db.rs` enables WAL but not `busy_timeout`. The CLI opens
  its own pool while the server holds one → `SQLITE_BUSY` on concurrent writes. Set
  `PRAGMA busy_timeout = 5000`.
- **P2-4. `deploy` GitHub-Pages path is fragile.** `deploy.rs:103-108` shells `rm -rf "{dir}/*"
  "{dir}/.*"`; `:127-136` runs `bash -c` with an interpolated commit message; worktree cleanup
  (`:139-142`) is skipped on the failure path, leaking `/tmp/oxibuilder-deploy-*`. Rewrite with guarded
  `std::fs` + guaranteed cleanup (`Drop`), or the `git2` crate.
- **P2-5. `deploy/` ops artifacts are absent.** `Dockerfile`, `Caddyfile.example`, launchd plist,
  systemd unit were removed during the SSG migration; `doc/08 §8.5` still references them.
- **P2-6. Rate limiter trusts `X-Forwarded-For`.** `rate_limit.rs:82-86` takes the first hop
  unverified — bypassable behind a proxy.
- **P2-7. Background scheduler has no timeout / error isolation.** `scheduler.rs` runs jobs
  sequentially; one panicking/slow job blocks the chain.
- **P2-8. No dependency audit in CI.** `ci.yml` runs no `cargo-audit`. Add a scheduled step.

## 5. P3 — Polish (non-blocking)

- **P3-1.** Zero frontend tests (`web/`, `admin-web/`). No SSG output assertion (the gap that let
  P0-1 ship).
- **P3-2.** `oxibuilder-ext-novels` (novels + chapters) has 1 integration test.
- **P3-3.** No structured logging / metrics endpoint (`http.rs` has `TraceLayer` text logs only).
- **P3-4.** `build_writer.rs:57-59` silently skips the SPA bundle if `web/dist` is absent — a build
  with no frontend produces a site with no JS and no warning.
- **P3-5.** `Markdown.tsx:8` renders owner markdown unsanitized (`dangerouslySetInnerHTML`).
  Single-owner assumption (doc §0.3) is reasonable; flagged for any future import/sync path.

## 6. Doc-vs-code discrepancies (consolidated)

| Doc | Says | Code reality |
|---|---|---|
| README:14, 207 | deploy to GitHub Pages / Cloudflare / Netlify | only github-pages works (`deploy.rs:25-27`) |
| README:53,56,88 | `oxibuilder-server` binary | does not exist → `oxibuilder-console` |
| README:31 | "v2 SSG in design" | implemented but broken (P0-1) |
| README:124, design §3.3, SKILL.md | `oxibuilder build` | requires `oxibuilder build build` (P1-1) |
| doc/06 Phase 6 | "⏳ planning complete" | implemented (broken) |
| doc/08 §8.9 | PAT scopes, `AdminAuth` enforcement | removed in `90b0140` |
| SKILL.md | `auth set/status/unset/token` | removed |
| docs/extension-sdk.md | `AdminAuth`, `oxibuilder-server` | removed |
| doc/08 §8.5 | `deploy/Dockerfile`, plist, systemd | directory absent |
| crates.io 0.4.0 | installable SPA-included crates | SPA gitignored, absent from package (P0-3) |

## 7. Verification methodology

- Read all 14 crates' source + tests; read `doc/00–08`, design spec, README, SDK/SKILL docs.
- 4 parallel read-only audits (SSG, security, ops, frontend/tests/CLI).
- **Executed** `cargo build --release -p oxibuilder` (✅ compiles), `cargo test --workspace`
  (**137 passed, 0 failed, 3 ignored**), `./target/release/oxibuilder build build` (❌ panics — P0-1),
  inspected `data/out/` (empty), `cargo package --list -p oxibuilder-core --no-verify` (❌ no SPA — P0-3),
  `git ls-files` (embedded-spa untracked).
- Cross-checked each scout claim against the file before recording it; the SSG scout's "build works"
  was **disproven by execution** — recorded the empirical result, not the read.
