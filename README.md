# Oxipage

> A self-hosted personal studio — a single place for the developer, writer, critic,
> and curator sides of "you".

Oxipage turns the friction of static-site blogging (open the repo → create a file →
write a commit message → push → wait for a build) into one CLI command, or one sentence
to an AI agent. It's not just a blog engine: each side of you — blog, novels, projects,
movie/book reviews, link curation, activity feed — lives in its own **extension**, and
they're all gathered into one **lobby**.

The deployable artifact is a **single Rust binary** with the frontend embedded, backed by
a single SQLite file. No separate database server, no orchestrator — run it on one Mac mini
(or any single Linux host) under launchd/systemd. Apple `container`/Docker is *optional*
packaging.

## Status

Foundation (Phase 0) through agent integration / API hardening (Phase 4) and most of OSS
productization (Phase 5) are implemented. The detailed, living tracker is
[`doc/08-remaining-implementation.md`](doc/08-remaining-implementation.md).

- **Core:** Axum HTTP server, SQLite (WAL) + per-extension namespaced migrations, FTS5 search
  (`tokenize='trigram'`), publish-time SSR snapshots, PAT scope auth, rate limiting,
  OpenAPI/Swagger UI, background-job scheduler.
- **9 extensions:** `profile` · `blog` · `projects` · `links` · `novels` · `movies` (TMDB) ·
  `books` (Aladin/Google Books) · `scraps` (HN/GeekNews) · `activity` (GitHub).
- **CLI:** `init` · `status` · `serve` · `auth` · `blog` · `project` · `link` · `lobby`.
  (CLI subcommands for the Phase 2 extensions — novel/review/scrap/activity — are deferred;
  those are reachable via API/web today.)
- **Verified:** `cargo test --workspace` **90 tests, 0 failed** · `cargo clippy --workspace
  --all-targets -- -D warnings` clean · `cd web && bun run build` OK.

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

# 2) Build the release binaries (server + CLI + all extensions)
cargo build --release
# → target/release/oxipage-server   (the server)
# → target/release/oxipage          (the CLI)
```

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

### 2. Start the server

```bash
./target/release/oxipage-server
# → listening on http://127.0.0.1:8787
```

Open **http://127.0.0.1:8787** — you'll see the lobby (read-only for visitors).

The server runs **with or without** an admin token. Without one it's fully read-only: every
write API returns `503 admin_not_configured`. To create content, generate a bootstrap admin
token and restart with it:

```bash
export OXIPAGE_ADMIN_TOKEN=$(openssl rand -hex 32)   # a random secret; keep it safe
./target/release/oxipage-server
```

### 3. Issue yourself a token (recommended over using the admin token directly)

The `OXIPAGE_ADMIN_TOKEN` is a bootstrap superuser (scope `admin`) meant for setup/recovery.
For everyday use, mint a scoped PAT and store it so you don't pass `--token` every time:

```bash
# Point the CLI at your server (defaults to http://127.0.0.1:8787, or [site].base_url)
export OXIPAGE_ENDPOINT=http://127.0.0.1:8787

# Create a PAT using the admin token, then save it to the credentials file (0600)
OXIPAGE_TOKEN=$OXIPAGE_ADMIN_TOKEN \
  ./target/release/oxipage auth token create --label owner --scopes admin
# → prints the plain token ONCE. Save it:
./target/release/oxipage auth set <plain-token>
./target/release/oxipage auth status   # → "a token is stored"
```

Now any CLI command reads the token from `~/.config/oxipage/credentials` (or `OXIPAGE_TOKEN`)
automatically. Token resolution order: `--token` flag → `OXIPAGE_TOKEN` env → credentials file.

### 4. Create and publish content

Everything starts as a **draft** (`published_at = NULL`). Publishing is always a separate,
explicit step — a safety guarantee that extends to AI agents.

```bash
oxipage blog new "Hello world" --lang en --file post.md --json    # → { "data": { "slug": "…" } }
oxipage blog list --draft                                          # see your drafts
oxipage blog publish <slug>                                        # live on the site

oxipage project add --title-ko "…" --title-en "My project" --tech rust --tech react --publish
oxipage link add --title "Cool site" --url https://example.com --featured
oxipage lobby layout projects --mode canvas                       # canvas | grid | list
oxipage status                                                     # server + content summary
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

Two paths (`crates/oxipage-core/src/auth.rs`):

| Path | Use | Storage |
|---|---|---|
| `OXIPAGE_ADMIN_TOKEN` | Bootstrap superuser (setup/recovery) | Environment variable (server) |
| PAT (`oxp_…`) | Everyday read/write, scoped | DB `auth_token`, SHA-256 hashed |

PAT scopes: `post:write` (create/edit drafts), `post:publish` (publish), `read` (read drafts),
`admin` (token management). The plain token is shown **once** at creation; thereafter only its
hash is stored and verified. Token-management API: `GET/POST /api/v1/auth/tokens`,
`DELETE /api/v1/auth/tokens/{id}` (all require `admin`).

## HTTP API

- Versioned prefix `/api/v1/**`; each extension mounts at `/api/v1/{extension}/**`.
- `GET /healthz` · `GET /api/v1/lobby/manifest` · `GET /api/v1/search?q=` ·
  `GET /api/v1/docs` (Swagger UI) · `GET /api/v1/docs/openapi.json`.
- Public reads need no auth (rate-limited to 120 req/min/IP); writes require a bearer token.

## Deployment

The default path is running the binary directly under launchd (macOS) or systemd (Linux).
Templates live in [`deploy/`](deploy/) (`oxipage.plist.example`, `oxipage.service.example`,
`Caddyfile.example`, `Dockerfile`) with full guidance in
[`doc/05-deployment-self-hosting.md`](doc/05-deployment-self-hosting.md). Expose it to the
internet with Cloudflare Tunnel (`cloudflared`) + a host-native Caddy reverse proxy — no
inbound ports opened on your home network.

## Project structure

```
oxipage/
├── crates/
│   ├── oxipage-core/          # runtime: HTTP, auth, search, scheduler, registry, snapshot, config
│   ├── oxipage-server/        # binary (oxipage-server) — statically links all extensions
│   ├── oxipage-cli/           # binary (oxipage) — the API's reference client
│   └── oxipage-ext-*/         # 9 extensions, each owning its DB, routes, CLI, background jobs
├── web/                       # React 19 + TS + Vite SPA, embedded via rust-embed
├── deploy/                    # Caddyfile / launchd plist / systemd unit / Dockerfile / deploy.yaml
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
cargo run -p oxipage-server          # backend :8787 (debug build reads web/dist from disk)
cd web && bun run dev                # frontend dev server :5173 (/api → :8787 proxy)

cargo test --workspace                                   # 90 tests
cargo clippy --workspace --all-targets -- -D warnings    # must be clean
```

See [`CONTRIBUTING.md`](CONTRIBUTING.md) for the full workflow and conventions.

## License

[MIT](LICENSE).
