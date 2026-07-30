# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.7.0] - 2026-07-30

### Added
- **Site-scoped console.** The console server now operates per-site rather than per-app. The router is mounted under `/api/console`; every extension handler resolves through `SiteScopedDb` middleware that picks the right `console.db` from the request's site slug.
- **Site directory wizard + `/s/{slug}/` redirect.** First-run setup walks the user through creating a site (slug, name, base URL, languages, enabled extensions). The `/s/{slug}/` URL scheme routes the SPA to the right site.
- **Site-scoped build/deploy/preview endpoints.** `POST /api/console/sites/{slug}/build` runs `oxipage build` against the site's working directory; `/deploy` and `/preview` follow the same site-scoped shape.
- **`create-site` CLI handler.** `oxipage init console` (or the new site-creation flow) creates the site directory, initializes `console.db`, registers the site, and surfaces the new slug in the site picker.
- **Console-only SPA in `web/src/admin/`.** The old `admin-web/` Vite app was folded into `web/src/admin/` (shared Tailwind v4 + OKLCH tokens). One `web/dist` build now ships both lobby and console surfaces.
- **`SitesFile` → `path-only` schema.** Sites are tracked as paths in `~/.config/oxipage/sites.toml`, no longer as opaque records.
- **`console.db` with `setup_state` table.** Per-site setup state lives next to the site, not in a global DB.

### Changed
- **BREAKING: `Extension::routes()` now returns `Router` (no state).** Extension handlers receive `Extension<SiteScopedDb>` instead of `State<AppState>`. First-party extensions were updated in the same commit; no third-party consumers exist (the ecosystem is in-tree only).
- **`oxipage-server` removed.** The old `:8788` admin module, the `admin-web/` app, and the standalone `admin` CLI subcommand were all deleted; the console absorbs their surface.

### Fixed
- Console router nested under `/api/console` so the SPA's static_handler no longer shadow-serves the API.
- E2E CLI tests use `--path` for the site root (post schema migration).
- `oxipage.toml` name + extensions corrupted from a merge smoke test — restored the original config.

### Style
- Workspace-wide `cargo fmt` pass (whitespace only).

## [0.6.0] - 2026-07-29

### Added
- **Per-extension setup sub-wizards.** Extensions that own external API keys (`movies`/TMDB, `books`/Aladin + Google Books, `activity`/GitHub) now declare their own multi-step `setup_wizard` instead of relying on a shared keys step.
  - `SetupFieldKind::Secret` for API keys (rendered as password, never prefilled).
  - Declarative `visible_when` rules (`VisibilityRule`) evaluated client-side via `visibility.ts` (`evalRule` / `mergeOutcome` / `resolvePrefill`).
  - **Action steps** run live checks after key entry: `movies_test` / `books_test` call the real external API and report `connection_ok`; `activity_sync` immediately syncs public GitHub activity (`repo::upsert`) and reports `synced`. Each action step shows conditionally on its key-step result.
- `ExtensionSubWizard` (admin-web): owns one extension's sub-wizard with internal step nav, outcome accumulation, and `visible_when` filtering. `GenericStep` now renders action steps (empty field sets) and `Secret`/password fields.
- `SetupSaveHandler::save` returns `StepOutcome` (values used to evaluate downstream `visible_when` / prefill); `setup_extension_step_handler` returns `DataEnvelope<StepOutcome>`.
- `persist_extension_config` is now `pub`, so each extension's key-step save handler persists its own config directly.

### Changed
- **`external_api_keys` mechanism removed.** The centralized `Extension::external_api_keys()` / `save_external_key()` trait methods, `ExternalApiKey` / `ExternalKeyScope` types, the `/setup/external-keys` route + handler, `StatusResult.external_api_keys`, the matching OpenAPI entries, and the front-end `ExternalKeysStep.tsx` + `submitExternalKeys` were all deleted. API keys now live in each extension's own `setup_wizard` key-step and persist via `persist_extension_config`. First-party-only extension ecosystem — no third-party consumers affected.
- **Setup wizard API renamed.** `Extension::setup_wizard_step(Option<SetupStep>)` → `setup_wizard(Option<ExtensionWizard { steps }>)`; `StatusResult.extension_steps` → `extension_wizards`; route `/setup/extension-step/{id}` → `/setup/extension-step/{ext_id}/{step_id}` (namespaced per extension).

### Fixed
- Clippy `-D warnings` lints in extension setup save handlers: `oxipage-ext-activity` (`collapsible_if` → let-chain), `oxipage-ext-books` / `oxipage-ext-movies` (redundant `matches!(.., Ok(_))` → `.is_ok()`).

## [0.5.0] - 2026-07-29

### Added
- **oxipage build** (SSG): full static-site build pipeline.
  - SSG regression test (`crates/oxipage-core/tests/ssg_build.rs`) asserts `out/` layout and asset references.
  - `BuildExt` trait now takes a `tokio::runtime::Handle` so `build_site` runs on rayon workers without panic.
  - `build_writer` sources the SPA from the embedded binary (no CWD `web/dist` dependency), writes `out/index.html`, `out/404.html`, and `out/assets/*` at root, and injects the real hashed script/css into content shells.
  - Extension static-data contract: `fetchBlogPost` loads `blog.json` and `.find(slug)`; `searchAll` client-filters the search index; collection cached at module scope.
  - `oxipage build build` flattened to `oxipage build`.

### Fixed
- **cli** (`oxipage-cli`): member-crate `[profile.release]` with `lto = true`, `codegen-units = 1`, `strip = "none"` so `cargo install oxipage` succeeds on macOS 27. The workspace root profile does not ship in the published `.crate` tarball; this restores the workaround for rust-lang/rust#157750 (mis-aligned LINKEDIT string pool in stripped proc-macro dylibs).

### Changed
- **README**: status section no longer says "v2 SSG in design"; install + getting started now reference `oxipage-console` (the binary that absorbed `oxipage-server` in doc/12); deploy claim now explicitly calls out the bailed Cloudflare Pages / Netlify paths.
- **extension-sdk docs**: replaced "AdminAuth" write-route rule with the loopback-only management-server model; `oxipage-server` crate references → `oxipage-console`.
- **oxipage-cli/SKILL.md**: replaced the removed PAT/auth flow with the loopback-only model.
- **oxipage-core/Cargo.toml**: added explicit `include = [...]` so `cargo publish` bundles `embedded-spa/`, `_registry.json`, and `_wasm-demo.wasm` (these are gitignored build artifacts; without `include`, `cargo install oxipage` would ship a placeholder SPA and a broken `oxipage build`).

### Security
- 17 outstanding `wasmtime 33.0.2` advisories remain, with two CRITICAL:
  - RUSTSEC-2026-0095 (Winch sandbox escape) — **does not apply**: workspace pin uses `features = ["cranelift", "runtime", "parallel-compilation", "cache"]`; the `winch` feature is not enabled, so the vulnerable code path is not compiled in.
  - RUSTSEC-2026-0096 (aarch64 Cranelift heap miscompile) — **conditional**: only affects 64-bit WebAssembly linear memories on aarch64; mitigated by default Spectre mitigations. Real on aarch64 deployments of the published CLI binary; out of scope for this release.
- wasmtime 33.x is no longer receiving security patches. Bumping to a supported major (36 / 42 / 43) is tracked separately; the bump cascades through the `oxipage-wasm` API surface (`Config, Engine, Instance, Linker, Module, Store, Caller, Val`) and needs its own release.

## [0.4.0] - 2026-07-29

### Changed
- **admin-web**: adopt Tailwind v4 + OKLCH design token pipeline shared with the public web app. Replaced hardcoded inline styles with utility classes across AdminShell, SiteSwitcher, content / dashboard / settings / extensions / themes pages; added ThemeToggle; extracted input/textarea primitives.

### Style
- Workspace-wide `cargo fmt` pass (whitespace only).

### Notes
- Effective crates.io floor is `0.3.0` (`oxipage-wasm@0.3.0` already published under tag v0.3.0); 0.4.0 advances past that burn.
- Continuation of the v0.3.0 line; the v0.3.0 Git tag was applied to a partial-publish state (4 crates were never released: `oxipage-ext-scraps`, `oxipage-ext-projects`, `oxipage-console`, `oxipage`). Those crates are not in 0.4.0 — they remain unpublished at 0.2.0 / absent from the registry; future cleanup is a separate concern.


[Unreleased]: https://github.com/a7garden/oxipage/compare/v0.7.0...HEAD
[0.7.0]: https://github.com/a7garden/oxipage/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/a7garden/oxipage/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/a7garden/oxipage/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/a7garden/oxipage/compare/v0.3.0...v0.4.0
