# Contributing

How to contribute to Oxipage. For basics, read the [`README`](README.md) first.

## Development environment

```bash
# Backend (:8787) + frontend dev server (:5173, /api → :8787 proxy)
cargo run -p oxipage-server &
cd web && bun install && bun run dev

# Or start the server via the CLI (`serve` is the one exception that boots the server process
# directly — doc/04 §4.1)
cargo run -p oxipage-cli -- serve
```

A `debug` build reads `web/dist` from disk, so run it alongside the frontend dev server. A
`release` build embeds `web/dist` into the binary via `rust-embed` at compile time, so
**`web/dist` must exist first** — run `cd web && bun run build` before a release build.

## Test / lint

```bash
cargo test --workspace                                   # 90 tests
cargo clippy --workspace --all-targets -- -D warnings   # must be clean
cargo fmt --all                                          # format
cd web && bun run build                                  # frontend typecheck + build
```

A PR must pass all four. Don't break dependency or test isolation.

## Project structure and boundaries

```
crates/
├── oxipage-core/      # shared runtime: HTTP, auth, search, scheduler, registry, snapshot, config
├── oxipage-server/    # binary (oxipage-server): statically links all extensions, assembles registry
├── oxipage-cli/       # binary (oxipage): the API's reference client (every command = an HTTP call)
└── oxipage-ext-*/     # 9 extensions. Each owns its DB tables, routes, CLI, background jobs.
```

**Core boundary:** an extension writes and reads **only its own tables**. It never JOINs another
extension's tables directly — compose via the core API (`lobby/manifest`, `search`) if needed
(doc/02 preamble).

Full architecture: [`doc/01-architecture.md`](doc/01-architecture.md); domain model:
[`doc/02-domain-model.md`](doc/02-domain-model.md). *(Those specs are in Korean — they're the
maintainer's internal design notes.)*

## Adding a new extension

[`docs/extension-sdk.md`](docs/extension-sdk.md) walks through it from scratch. Summary:

1. Create `crates/oxipage-ext-<id>/` and add it to the workspace `members`.
2. Implement the `Extension` trait (`id`/`display_name`/`migrations`/`routes`/`lobby_summary`/…).
3. **Register it in the server:** add a line to `all_extensions()` in
   `crates/oxipage-server/src/lib.rs`.
   > ⚠ Rewrite `all_extensions()` **wholesale with `write`** — partial `edit`/`SWAP` keeps
   > dropping or duplicating `vec![` (doc/08 §8.9).
4. Add metadata to `registry/index.json`.

Reference implementations: `oxipage-ext-blog` (simple), `oxipage-ext-projects` (forced bilingual
+ screenshots), `oxipage-ext-movies` (TMDB integration + SeriesGroup).

## Key conventions (implementation deviations — must follow)

These are concrete implementation contracts that **differ slightly from the design docs** (doc/).
Original list: doc/08 §8.9.

| Area | Rule |
|---|---|
| **Rating** | Stored as an integer `0`–`10` (`/2` → `0.0`–`5.0`, 5 stars). The "0–20" in doc/02 §2.1 was a typo; code + docs use `0–10`. |
| **axum path params** | `{slug}` form (not `:slug`). No trailing slash. axum 0.8 semantics. |
| **`order` reserved word** | Always use `display_order` (`order` is a SQL reserved word). |
| **Draft-first** | `create` always sets `published_at = NULL`. Publishing is a separate `POST /{id}/publish` action only. Same for the CLI (`add`/`new` = draft). |
| **Write-route auth** | A handler taking `_auth: AdminAuth` enforces `post:write` at entry. Publish actions call `auth.require_scope("post:publish")?;` first. Token management calls `require_scope("admin")?`. |
| **PAT vs ADMIN_TOKEN** | `OXIPAGE_ADMIN_TOKEN` = superuser (scopes `["admin"]`). PATs have `post:write`/`post:publish`/`read`. |
| **FTS5 shared index** | On publish, upsert into `search_documents` via `oxipage_core::search::upsert(...)`. On delete/disable, immediately `delete`/`delete_extension` (doc/02 §2.13). |
| **External API keys** | Store only the **env-var name** in `oxipage.toml [integrations]` (never the plaintext key). Read via `Config::integrations` helpers (`tmdb_key()`/`aladin_key()`/`github_username()`). If a key is absent, that integration silently disables (doc/01 §1.9). |
| **OpenAPI / SSR** | Hand-rolled `serde_json` spec and hand-rolled templates instead of `utoipa`/Askama (saves dependencies; fine at single-user scale). |
| **CLI flag quirks** | `auth token create` uses `--scopes` (plural, comma-separated). `lobby layout` takes the extension as a positional: `oxipage lobby layout <ext> --mode <m>` (no `set`). Multi-value args like `--tech` repeat: `--tech rust --tech react`. |
| **macOS 27 build** | The release profile pins `strip = "none"` (rust-lang/rust#157750). New crates inherit it. |

## Working with agents

When an AI agent (oh-my-pi, etc.) manages content via the CLI, the safety rules and workflow are
in [`.agent/skills/oxipage-cli/SKILL.md`](.agent/skills/oxipage-cli/SKILL.md). Core rule:
**drafts are automatic, but publishing (`publish`/`--publish`) always requires explicit human
approval.** Give agent tokens only `post:write`, never `post:publish`.

## Commits / PRs

- Message format `type(scope): subject` recommended (e.g. `feat(ext-movies): series group rating`).
  See existing history (`git log --oneline`).
- If you make an implementation decision that diverges from the design docs, add a line to
  doc/08 §8.9 (key design deviations).
