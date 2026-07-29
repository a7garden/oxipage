# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0] - 2026-07-29

### Changed
- **admin-web**: adopt Tailwind v4 + OKLCH design token pipeline shared with the public web app. Replaced hardcoded inline styles with utility classes across AdminShell, SiteSwitcher, content / dashboard / settings / extensions / themes pages; added ThemeToggle; extracted input/textarea primitives.

### Style
- Workspace-wide `cargo fmt` pass (whitespace only).

### Notes
- Effective crates.io floor is `0.3.0` (`oxipage-wasm@0.3.0` already published under tag v0.3.0); 0.4.0 advances past that burn.
- Continuation of the v0.3.0 line; the v0.3.0 Git tag was applied to a partial-publish state (4 crates were never released: `oxipage-ext-scraps`, `oxipage-ext-projects`, `oxipage-console`, `oxipage`). Those crates are not in 0.4.0 — they remain unpublished at 0.2.0 / absent from the registry; future cleanup is a separate concern.

[Unreleased]: https://github.com/a7garden/oxipage/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/a7garden/oxipage/compare/v0.3.0...v0.4.0
