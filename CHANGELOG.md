# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

### Style
- Workspace-wide `cargo fmt` pass (whitespace only).

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

[Unreleased]: https://github.com/a7garden/oxipage/compare/v0.5.0...HEAD
[0.5.0]: https://github.com/a7garden/oxipage/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/a7garden/oxipage/compare/v0.3.0...v0.4.0
