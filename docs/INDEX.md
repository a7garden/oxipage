# oxibuilder — Documentation Index

> Oxibuilder = multi-extension personal site. Two doc trees below; the larger is `docs/`.
> Canonical unified design system: `project-oxi/.github/DESIGN.md` (per-repo pointer: `.omp/DESIGN-REF.md`).

---

## Canonical design system

- **Source of truth:** `project-oxi/.github/DESIGN.md` (v1.0 · 2026-07-31)
- **Per-repo pointer:** `.omp/DESIGN-REF.md` — role + adaptation + migration status
- **Oxibuilder-specific surfaces:** `doc/UNIFIED-DESIGN.md` — lobby 3 modes, rating gold, 6 public themes, migration path

---

## `doc/` — Project domain docs (15 files, no subdirs)

Linear doc series: domain model, CLI/SDK, deployment, console, roadmap, remaining work.

| File | Purpose |
|------|---------|
| `00-overview.md` | Project overview, audience, non-goals |
| `01-architecture.md` | Workspace layout, extension boundaries, build graph |
| `02-domain-model.md` | Sites, extensions, snippets, content schemas |
| `03-design-system.md` | (Legacy per-repo design narrative — superseded by `doc/UNIFIED-DESIGN.md` + canonical) |
| `04-cli-api-skill.md` | `oxibuilder` CLI surface + AI skill contract |
| `05-deployment-self-hosting.md` | Deploy targets, self-host knobs, env vars |
| `06-roadmap.md` | Public roadmap, milestone ordering |
| `07-remaining-work.md` | Known gaps, deferred scope |
| `08-remaining-implementation.md` | Line-level remaining-implementation list |
| `09-multi-site.md` | Multi-site orchestration model |
| `10-cli-hardening.md` | CLI error paths, exit codes, recovery |
| `11-cli-extensibility.md` | Extension authoring hook points |
| `12-console.md` | Admin/console architecture |
| `13-first-run-ux.md` | First-run wizard + onboarding flow |
| `UNIFIED-DESIGN.md` | **Project-specific design adaptation** (keep; canonical pointer added) |

---

## `docs/` — Production, accessibility, superpowers plans/specs (53 files)

| Subdir | Files | Purpose |
|--------|-------|---------|
| `.` | 7 | Top-level public-facing docs (INDEX, README, accessibility, extension-sdk, production-design, production-readiness-report, wasm-spike) |
| `superpowers/` | 3 | Daily progress logs (console suite, console remaining) |
| `superpowers/plans/` | 21 | Implementation plans (Phase 0-bones, console subprojects, deploy pipeline, theme system, etc.) |
| `superpowers/specs/` | 21 | Design specs paired with plans (design-system v2, console rename, runtime routing, etc.) |
| `archive/transient/` | 1 | One-off status snapshots (`.release-prep-status.md`) |

### Top-level (`docs/`)

| File | Purpose |
|------|---------|
| `README.md` | Docs-tree entry + reading order |
| `accessibility.md` | Accessibility commitments, APCA contrast targets, motion safety |
| `extension-sdk.md` | Public Extension SDK contract (manifest, hooks, build targets) |
| `production-design.md` | Production-track UX/UI design |
| `production-readiness-report.md` | Pre-launch readiness audit |
| `wasm-spike.md` | WASM integration spike notes |
| `INDEX.md` | This file |

### `docs/superpowers/` — progress logs

| File | Date | Purpose |
|------|------|---------|
| `2026-07-30-console-remaining-implementation.md` | 2026-07-30 | Console remaining work log |
| `2026-07-30-console-remaining-subprojects.md` | 2026-07-30 | Console remaining subprojects log |
| `2026-07-31-console-suite-progress.md` | 2026-07-31 | Console suite roll-up |

### `docs/superpowers/plans/` — implementation plans

| File | Date | Purpose |
|------|------|---------|
| `2026-07-27-phase-0-bones.md` | 2026-07-27 | Phase 0 scaffold |
| `2026-07-28-console-rename.md` | 2026-07-28 | Console naming + identity |
| `2026-07-28-first-run-ux.md` | 2026-07-28 | First-run UX plan |
| `2026-07-28-ssg-implementation.md` | 2026-07-28 | SSG implementation plan |
| `2026-07-29-extension-native-treatment.md` | 2026-07-29 | Extension native treatment |
| `2026-07-29-extension-wizard-subwizards.md` | 2026-07-29 | Extension wizard subwizards |
| `2026-07-30-console-api-wiring-and-editor-screens.md` | 2026-07-30 | Console API + editor screens |
| `2026-07-30-console-deploy-pipeline-plan.md` | 2026-07-30 | Deploy pipeline plan |
| `2026-07-30-console-extension-gaps-plan.md` | 2026-07-30 | Console extension gaps plan |
| `2026-07-30-console-global-ux-plan.md` | 2026-07-30 | Console global UX plan |
| `2026-07-30-console-settings-residual-plan.md` | 2026-07-30 | Console settings residual plan |
| `2026-07-30-console-shell-redesign.md` | 2026-07-30 | Console shell redesign plan |
| `2026-07-30-site-picker-console.md` | 2026-07-30 | Site picker console |
| `2026-07-30-site-picker-console-remaining.md` | 2026-07-30 | Site-picker console remaining |
| `2026-07-31-admin-theme-system-plan.md` | 2026-07-31 | Admin theme system plan |
| `2026-07-31-console-followup-plan.md` | 2026-07-31 | Console followup plan |
| `2026-07-31-console-preview-media-plan.md` | 2026-07-31 | Console preview/media plan |
| `2026-07-31-console-runtime-routing-foundation-plan.md` | 2026-07-31 | Runtime routing foundation plan |
| `2026-07-31-extension-authoring-ux-plan.md` | 2026-07-31 | Extension authoring UX plan |
| `2026-07-31-github-pages-console-deploy-plan.md` | 2026-07-31 | GitHub Pages deploy plan |
| `2026-08-01-inline-media-authoring-plan.md` | 2026-08-01 | Inline media authoring plan |

### `docs/superpowers/specs/` — paired design specs

| File | Date | Purpose |
|------|------|---------|
| `2026-07-27-design-system-v2-design.md` | 2026-07-27 | Design system v2 spec |
| `2026-07-28-console-rename-design.md` | 2026-07-28 | Console rename spec |
| `2026-07-28-static-site-generator-design.md` | 2026-07-28 | SSG design |
| `2026-07-29-extension-native-treatment-design.md` | 2026-07-29 | Extension native treatment spec |
| `2026-07-29-extension-wizard-subwizards-design.md` | 2026-07-29 | Extension wizard subwizards spec |
| `2026-07-30-console-data-foundation-design.md` | 2026-07-30 | Console data foundation spec |
| `2026-07-30-console-deploy-pipeline-design.md` | 2026-07-30 | Deploy pipeline spec |
| `2026-07-30-console-extension-gaps-design.md` | 2026-07-30 | Extension gaps spec |
| `2026-07-30-console-global-ux-design.md` | 2026-07-30 | Global UX spec |
| `2026-07-30-console-settings-residual-design.md` | 2026-07-30 | Settings residual spec |
| `2026-07-30-console-shell-redesign.md` | 2026-07-30 | Console shell design |
| `2026-07-30-site-picker-console-design.md` | 2026-07-30 | Site picker console spec |
| `2026-07-31-admin-theme-system-design.md` | 2026-07-31 | Admin theme system spec |
| `2026-07-31-console-auto-fixes.md` | 2026-07-31 | Console auto-fixes spec |
| `2026-07-31-console-followup-design.md` | 2026-07-31 | Console followup design |
| `2026-07-31-console-preview-media-design.md` | 2026-07-31 | Preview/media spec |
| `2026-07-31-console-reliability-publishing-suite-design.md` | 2026-07-31 | Reliability + publishing suite spec |
| `2026-07-31-console-runtime-routing-foundation-design.md` | 2026-07-31 | Runtime routing foundation spec |
| `2026-07-31-extension-authoring-ux-design.md` | 2026-07-31 | Extension authoring UX spec |
| `2026-07-31-github-pages-console-deploy-design.md` | 2026-07-31 | GitHub Pages deploy spec |
| `2026-08-01-inline-media-authoring-design.md` | 2026-08-01 | Inline media authoring spec |

### `docs/archive/transient/`

| File | Purpose |
|------|---------|
| `.release-prep-status.md` | One-off release-prep status snapshot (moved 2026-08-02) |

---

## Reading order

1. `README.md` → `00-overview.md` → `01-architecture.md`
2. `02-domain-model.md` → `04-cli-api-skill.md`
3. `03-design-system.md` + `doc/UNIFIED-DESIGN.md` + `.omp/DESIGN-REF.md` (canonical pointer)
4. `05-deployment-self-hosting.md` → `09-multi-site.md`
5. `docs/extension-sdk.md` → `docs/accessibility.md` → `docs/production-design.md`
