# Oxipage

> A personal site generator for humans and AI agents — blog, portfolio, reviews, novels,
> and more, all from one CLI.

Oxipage turns the friction of static-site blogging (open the repo → create a file →
write a commit message → push → wait for a build) into one CLI command, or one sentence
to an AI agent. It's not just a blog engine: each side of you — blog, novels, projects,
movie/book reviews, link curation, activity feed — lives in its own **extension**, and
they're all gathered into one **lobby**.

**Oxipage is a Static Site Generator.** Content is managed through a CLI (or AI agent)
against a local SQLite database. `oxipage build` generates static HTML + JSON + JS files,
and `oxipage deploy` pushes them to GitHub Pages, Cloudflare Pages, or Netlify. No runtime
server needed for the public site — zero operating cost, zero security surface.

## Status

Foundation (Phase 0) through OSS productization (Phase 5) are implemented. The **v2 SSG
pivot** (Phase 6) is implemented on this branch — `BuildExt` trait, rayon parallel build
pipeline, `oxipage deploy` GitHub Pages target, `oxipage query/schema` for AI agents,
React SPA → static JSON data. See [`docs/production-readiness-report.md`](docs/production-readiness-report.md).

- **Management server:** `oxipage console` (binary = `oxipage-console`) — Axum, SQLite (WAL) +
  per-extension namespaced migrations, FTS5 search (`tokenize='trigram'`), publish-time SSR
  snapshots, local-only (no auth — bind to `127.0.0.1`), rate limiting, OpenAPI/Swagger UI,
  background-job scheduler (cron-driven).
- **Static site generator:** `oxipage build` → `out/` (HTML + JSON + hashed assets, sources
  the SPA from the embedded binary) → `oxipage deploy --target github-pages`.
- **9 extensions:** `profile` · `blog` · `projects` · `links` · `novels` · `movies` (TMDB) ·
  `books` (Aladin/Google Books) · `scraps` (HN/GeekNews) · `activity` (GitHub).
- **CLI:** `init` · `status` · `console` · `blog` · `project` · `link` · `lobby` ·
  `extension` · `site` · `admin` · `backup` · `build` · `deploy` · `query` · `schema` · `cache refresh`.
- **Verified:** `cargo test --workspace` **139 passed / 0 failed** (3 ignored — platform-specific) ·
  `cargo clippy --workspace --all-targets -- -D warnings` clean · `cd web && bun run build` OK.

## Requirements

- **Rust 1.96+** (stable, edition 2024)
- **bun 1.3+** — frontend build only. No Node at runtime (the bundle is statically embedded).

## Install

Oxipage is currently **build-from-source** (prebuilt binaries and a one-line install script
are Phase 5 / future `oxipage-starter` repo).

```bash
git clone https://github.com/oxipage/oxipage.git
cd oxipage

# 1) Build the frontend (embedded into the binary at compile time — do this first)
cd web && bun install && bun run build && cd ..

# 2) Build the release binaries (management server + CLI + all extensions)
cargo build --release
# → target/release/oxipage-console  (the local management server: API + admin-web UI)
# → target/release/oxipage          (the CLI: content management + build + deploy + query)

> **macOS 27 note:** the release profile pins `strip = "none"`. macOS 27's dyld rejects the
> mis-aligned string pool of stripped Mach-O dylibs (rust-lang/rust#157750), so the default
> `debuginfo` stripping would break proc-macro loading.

## Getting started

### 1. Configure

Copy the example config (which enables **all** extensions) and edit `[site]`:

```bash
cp oxipage.toml.example oxipage.toml
$EDITOR oxipage.toml
```

Key fields:
- `[site].base_url` — your public domain (the CLI defaults its endpoint to this).
- `[site].default_lang` — the lobby's default language (`"ko"` or `"en"`).
- `[extensions].enabled` — `[]` (empty) enables every compiled-in extension; list specific IDs
  (e.g. `["profile", "blog"]`) to enable only those. *(Note: `oxipage init` is a minimal
  alternative that scaffolds a **profile-only**, Korean-default config — most users will want
  the `.example` above instead.)*

Secrets (API keys) are **never** stored in the config file — only the *names* of the environment
variables that hold them. See [`oxipage.toml.example`](oxipage.toml.example).

### 2. Start the management server
./target/release/oxipage-console
# → listening on http://127.0.0.1:8787
#   (admin-web + API — content management only)
```

Open **http://127.0.0.1:8787** — you'll see the admin console.

### 3. Authentication (local-only)

The management server runs **without authentication** by design — it is intended to be bound
to `127.0.0.1` and never exposed to the public internet (see [Deployment](#deployment)).
The `oxipage` CLI still accepts `--token` / `OXIPAGE_TOKEN` for symmetry with future remote
servers, but the local server does not enforce it. Do not bind the management server to a
public interface without putting a reverse-proxy auth layer in front of it.

Token resolution order for the CLI: `--token` flag → `OXIPAGE_TOKEN` env → credentials file
at `~/.config/oxipage/credentials` (0600).

### 4. Create content and build

Everything starts as a **draft** (`published_at = NULL`). Publishing is always a separate,
explicit step — a safety guarantee that extends to AI agents.

```bash
oxipage blog new "Hello world" --lang en --file post.md --json    # → { "data": { "slug": "…" } }
oxipage blog list --draft                                          # see your drafts
oxipage blog publish <slug>                                        # mark as published

oxipage project add --title-ko "…" --title-en "My project" --tech rust --tech react --publish
oxipage link add --title "Cool site" --url https://example.com --featured
oxipage lobby layout projects --mode canvas                       # canvas | grid | list
```

### 5. Build and deploy the static site

```bash
oxipage build                         # generates out/ (HTML + JSON + JS + images)
oxipage deploy --target github-pages  # pushes out/ to gh-pages branch
# → site is live at your GitHub Pages URL
```

Pass `--json` to any command for machine-parseable output (this is how AI agents consume it).

### Talking to a remote server

For a deployed instance, set the endpoint and a token in your shell (or CI/agent environment):

```bash
export OXIPAGE_ENDPOINT=https://your-domain.com
export OXIPAGE_TOKEN=<your PAT>
oxipage status --json
```

## Configuration

`oxipage.toml` (defaults are used if absent). Override with environment variables:
`OXIPAGE_CONFIG`, `OXIPAGE_PORT`, `OXIPAGE_DATA_DIR`. Full reference:
[`oxipage.toml.example`](oxipage.toml.example).

```toml
[site]
name = "My Oxipage"
base_url = "https://example.com"     # public domain; CLI defaults to this
default_lang = "ko"                   # "ko" | "en"
languages = ["ko", "en"]

[server]
host = "127.0.0.1"                    # 0.0.0.0 behind a reverse proxy
port = 8787
data_dir = "data"                     # SQLite db + media; prefer an absolute path in prod

[extensions]
enabled = []                          # empty = all compiled-in extensions active

[integrations]
# github_username = "yourname"                    # activity (public Events API)
# tmdb_api_key_env = "OXIPAGE_TMDB_KEY"           # movies (manual mode if unset)
# aladin_ttbkey_env = "OXIPAGE_ALADIN_TTBKEY"     # books (Google Books fallback if unset)

[lobby]
default_mode = "grid"                 # "canvas" | "grid" | "list"; per-extension via API/CLI
```

External integrations **silently disable themselves** when their key is absent, so you only
wire up the ones you want.

## Authentication

The management server (`oxipage` console / `oxipage serve`) is **local-only** and runs without
enforced authentication. It is intended to be bound to `127.0.0.1` and never exposed to the
public internet (see [Deployment](#deployment)). If you must expose it, put a reverse-proxy
auth layer (mTLS, basic auth, OAuth proxy) in front of it.

The CLI accepts `--token` / `OXIPAGE_TOKEN` for symmetry with future remote servers, but the
local server does not enforce it. The GitHub activity webhook
(`POST /api/v1/activity/webhook`) **is** public and verifies requests with an HMAC-SHA256
signature (`X-Hub-Signature-256`). Set `OXIPAGE_GITHUB_WEBHOOK_SECRET` to the secret you
configured in your GitHub webhook settings; if unset, the endpoint returns 503.

## HTTP API

- Versioned prefix `/api/v1/**`; each extension mounts at `/api/v1/{extension}/**`.
- `GET /healthz` · `GET /api/v1/lobby/manifest` · `GET /api/v1/search?q=` ·
  `GET /api/v1/docs` (Swagger UI) · `GET /api/v1/docs/openapi.json` ·
  `POST /api/v1/backup/snapshot`.

## Deployment

Oxipage is a **Static Site Generator**. The public site needs no runtime server.

### Deploy via the CLI (recommended)

```bash
oxipage build
oxipage deploy --target github-pages
This pushes `out/` to the `gh-pages` branch of your repo. Your GitHub Pages URL will serve
the site immediately. Cloudflare Pages (`--target cloudflare`) and Netlify (`--target netlify`)
are tracked but not yet implemented (`deploy.rs` will refuse with `"<target> not yet
implemented"`).

Before deploying, preview the static site locally:

```bash
oxipage console --preview
# → http://127.0.0.1:8787 serves out/
```

### Management server (localhost only)

The content management server (`oxipage console`) still runs locally for the admin-web UI and
API. This server is never exposed to the internet.

### Data backups

Your content lives in the SQLite database. Back it up at any time:

```bash
oxipage backup snapshot
# → data/backups/oxipage-<epoch>.db
```

Media files under `data/media/` need a separate backup (rsync, restic, etc.).

## Project structure

```
oxipage/
├── crates/
│   ├── oxipage-core/          # management + build: HTTP, search, scheduler, registry, BuildExt, build pipeline
│   ├── oxipage-console/       # binary (oxipage-console) — the local management server (admin-web + API)
│   ├── oxipage-cli/           # binary (oxipage) — content management + build + deploy + query
│   └── oxipage-ext-*/         # 9 extensions, each owning its DB, routes, CLI, BuildExt
├── web/                       # React 19 + TS + Vite SPA, static JSON data layer
├── deploy/                    # deploy config templates
├── registry/                  # curated extension index (JSON)
├── doc/                       # design spec (Korean, internal) — 00 … 08
├── docs/                      # implementation/ops notes (English)
└── .agent/skills/oxipage-cli/ # agent skill for oh-my-pi etc.
```

## Documentation

- **[`doc/`](doc/)** — the **design specification** (Korean, internal working spec). `00-overview` →
  `06-roadmap` is the core design; `07`/`08` are phase trackers. Start at the
  [document map](doc/00-overview.md). *(English translation of the spec is not planned — it's the
  maintainer's working design notes; the English surface is this README + `docs/` + CONTRIBUTING.)*
- **[`docs/`](docs/)** — implementation/ops notes (English): [accessibility measurements](docs/accessibility.md),
  [extension SDK guide](docs/extension-sdk.md).
- **[`CONTRIBUTING.md`](CONTRIBUTING.md)** — dev workflow, testing, adding an extension, key conventions.
- **[`.agent/skills/oxipage-cli/SKILL.md`](.agent/skills/oxipage-cli/SKILL.md)** — CLI skill for AI coding agents.

## Development

```bash
cargo run -p oxipage-console       # backend :8787 (debug build reads web/dist from disk)
cd web && bun run dev              # frontend dev server :5173 (/api → :8787 proxy)

cargo test --workspace                                   # 139 tests
cargo clippy --workspace --all-targets -- -D warnings    # must be clean
```

See [`CONTRIBUTING.md`](CONTRIBUTING.md) for the full workflow and conventions.

## License

[MIT](LICENSE).
