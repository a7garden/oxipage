# Static Mounts — CLI Management (Agent-First)

**Status:** Ready for implementation
**Date:** 2026-08-06
**Depends on:** `2026-08-06-static-mounts-design.md` (merged @ af6b96f) — mounts already
exist as `[[mounts]]` in `oxibuilder.toml`, copied into `out/{path}/` at build time and
shown as lobby link cards.

## 1. Problem

Static mounts shipped config-driven, but there is **no CLI command** to manage them. The
oxibuilder CLI is the API reference client (doc/04 §4.1): every command is an authenticated
HTTP call to the running console, with the sole exceptions of `console` (boots the server)
and `build` (standalone SSG). For an agent to "manage everything via CLI" — the stated goal,
CLI-before-GUI — mounts must be manageable through the same HTTP path the agent already uses
for `lobby` / `site` / `deploy`.

## 2. Decision

**Approach A (settings-API, HTTP).** Add dedicated per-site mount endpoints that operate on
`oxibuilder.toml` directly (the single source of truth for all settings). The CLI calls them.
This mirrors `config_put`'s existing patch-doc → validate → atomic-write → reload flow and
leaves the future admin GUI reusing the identical API.

Rejected:
- **Direct toml editing in the CLI** — a new filesystem-editing pattern that diverges from
  every settings command, leaves a running console stale until restart, and gives the GUI no
  shared path.
- **DB table (like `blog`/`link`)** — diverges from the just-merged toml-as-truth design and
  needs a migration + AppState-rebuild path; the win (offline editing) is marginal since the
  console runs for preview/deploy anyway.

## 3. Architecture

### 3.1 Source of truth — unchanged

`oxibuilder.toml` `[[mounts]]` remains the only store. `MutableSiteSettings.mounts` keeps
`#[serde(skip)]` (mounts stay **off** the `/config` JSON surface — they get their own
resource, exactly as `theme` and `lobby` have dedicated endpoints separate from `/config`).
`Config::load` keeps resolving relative→absolute sources at load; `ctx.settings.mounts`
(resolved) continues to feed the three build paths unchanged.

### 3.2 Three new per-site endpoints

Mounted on the per-site router (`per_site::per_site_router`) alongside `/config`, `/theme`,
`/build` — i.e. under `/s/{slug}/mounts`:

| Method + path | Action |
|---|---|
| `GET  /s/{slug}/mounts` | List mounts. **Reads the raw toml doc** (not `ctx.settings`) so the returned `source` matches what the user wrote (e.g. `"../portfolio"`), not the build-resolved absolute path. |
| `POST /s/{slug}/mounts` | Add a mount. Patches the raw doc's `[[mounts]]` array (append), validates, atomic-writes, reloads. 400 on any validation failure. |
| `DELETE /s/{slug}/mounts/{id}` | Remove the mount whose `id` matches. 404 if absent. Patches doc, validates, writes, reloads. |

All three hold `ctx.config_write_lock` across the read-modify-write (same serialization as
`config_put`) so concurrent mount/config writes cannot clobber each other.

### 3.3 Shared persist helper (refactor)

Extract `config_put`'s tail (lines ~260-296: serialize → `Config::from_toml_str` validate →
temp-file + rename atomic write → `Config::load` reload →
`*ctx.settings.write() = from_config(new_cfg)`) into:

```rust
async fn persist_toml_and_reload(
    ctx: &SiteContext,
    doc: &toml::Value,
) -> Result<(), (StatusCode, String)>
```

`config_put` and the three mount handlers all call it. Single source of truth for "make a
patched toml durable and refresh the in-memory snapshot".

### 3.4 Raw-doc patching

Mounts are read/written through the `toml::Value` document tree, **never** through the
resolved `Config`/`MutableSiteSettings`. This preserves comments, unknown sections, and
formatting (the established `config_put` discipline) and keeps the stored `source` verbatim.
A `[[mounts]]` entry serializes as a TOML table:

```toml
[[mounts]]
id            = "portfolio"
source        = "../portfolio"
path          = "portfolio"
title_ko      = "포트폴리오"
title_en      = "Portfolio"
description   = "Hand-crafted work"
icon          = "🖼️"
open_in_new_tab = false
```

## 4. CLI

New `mount` subcommand, registered on the root `Command` enum, dispatched like `lobby`:

```
oxibuilder mount add \
  --id portfolio --source ../portfolio --path portfolio \
  [--title-ko 포트폴리오] [--title-en Portfolio] \
  [--desc "..."] [--icon 🖼️] [--new-tab]
oxibuilder mount list            # honors --json
oxibuilder mount rm <id>
```

- Every subcommand is an HTTP call to `/s/{slug}/mounts` (the client resolves the active
  site slug exactly as it does for `/config`).
- `--json` output is honored (agent-readable), consistent with the rest of the CLI.
- **Client-side pre-check** for immediate UX feedback: reject empty `id`/`source`/`path`, a
  `path` using a reserved prefix (`assets,data,media,api,search,s,admin,lobby,theme`), or a
  duplicate `id` against the live `list` — before hitting POST. The server remains
  authoritative (re-runs `validate_mounts`).

## 5. Validation & errors

Server-side reuses the existing `validate_mounts` (reserved prefixes, duplicate ids, invalid
path characters). HTTP status mapping:

| Condition | Status |
|---|---|
| reserved prefix / dup id / bad path / missing required field | `400` (body = reason) |
| `rm` unknown id | `404` |
| toml read/parse/write or reload failure | `500` |

## 6. Testing

- **Core:** `validate_mounts` already covered by the merged `static_mounts.rs`. No new core
  tests unless a gap surfaces.
- **Endpoints:** add → list (raw source preserved) → rm → list-empty round-trip; POST rejects
  reserved prefix and duplicate id (400); rm unknown id (404); concurrency not tested
  (lock is straightforward).
- **CLI smoke:** `mount add` then `mount list --json` shows it; `mount rm` removes it. Run
  against a booted console on a temp data_dir.

## 7. Non-goals

- Admin GUI for mounts (the same endpoints serve it later).
- `mount edit` (mutate an existing entry) — `rm` + `add` covers it for now; add if asked.
- Moving source resolution to build-time (not needed — raw-doc patching keeps stored sources
  verbatim while `Config::load` continues to resolve for builds).

## 8. Follow-on chain (this session)

1. Implement §3-§6.
2. `cargo test` + `clippy -D warnings` + `bun run build && build:static` green.
3. Local-install the `oxibuilder` binary; verify on `PATH`.
4. Author the `oxibuilder-blog` managed skill (the CLI-driven blog-creation recipe) and
   install it into OMP.
5. Use the skill to start the user's blog.
