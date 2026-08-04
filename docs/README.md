# Oxibuilder documentation

Oxibuilder's documentation is split across two directories.

| Location | Role | Audience |
|---|---|---|
| [`../doc/`](../doc/) | **Design spec** — `00-overview` through `08-remaining-implementation`: vision, architecture, domain model, design system, CLI/API, deployment, roadmap. Start at the [document map](../doc/00-overview.md). *(Korean — the maintainer's internal working spec.)* | Anyone understanding the design |
| [`./`](.) (this dir) | **Implementation/ops notes** — measurements and guides accumulated as the design is implemented. | Implementers / contributors |

## Implementation / ops notes

- **[accessibility.md](accessibility.md)** — WCAG 2.1 AA contrast-ratio measurements. Converts the
  OKLCH tokens in `web/src/shared/tokens.css` to sRGB and records the per-pair ratios for light/dark
  modes, plus the 2026-07-27 token adjustments (`--p-gold-600`, dark `--color-text-tertiary`).
- **[extension-sdk.md](extension-sdk.md)** — a from-scratch guide to building a new extension:
  crate scaffold, the `Extension` trait, core rules (FTS5 / draft-first / auth / `display_order`),
  server registration, test patterns.

## Agent skill

- **[`../.agent/skills/oxibuilder-cli/SKILL.md`](../.agent/skills/oxibuilder-cli/SKILL.md)** — the CLI
  skill read by AI coding agents (oh-my-pi, etc.). Covers the draft-first principle, the auth flow,
  and example workflows.

## Contributing

Development workflow, testing, adding an extension, and key conventions are in
[`../CONTRIBUTING.md`](../CONTRIBUTING.md).
