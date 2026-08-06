# Static Mounts Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let users graft an external static directory (hand-written HTML or pre-built output) into the oxibuilder site at a configurable URL prefix, discoverable as a lobby link card.

**Architecture:** Config-driven (`[[mounts]]` in `oxibuilder.toml`). At build time, a new raw-directory-copy step in `write_build_output` materializes each mount under `out/{path}/`. The lobby manifest gains a `mounts` array (fed from config in the single `manifest::assemble()`), and the SPA lobby renders plain-anchor link cards (full-page navigation to the standalone mount HTML). No DB, no new CLI subcommand, no live-console serving (the lobby is only ever served from the built `out/`, where mounts already exist).

**Tech Stack:** Rust 1.96 (edition 2024), axum/tower, serde, rayon build pipeline; React 19 + TS + Vite SPA; SQLite. Spec: `docs/superpowers/specs/2026-08-06-static-mounts-design.md`.

## Global Constraints

- **Rust 1.96+**, edition 2024. `cargo clippy --workspace --all-targets -- -D warnings` MUST stay clean.
- **Single binary, Node-zero at runtime.** Mounts are a file-copy only — no shelling out to external build tools.
- `cargo test --workspace` MUST stay green (currently 139 passing).
- `cd web && bun run build` MUST stay green (tsc + bundle).
- Mounts copy **after** the `write_build_output` `out/` wipe (step 1) — they are written in a late step so the wipe never destroys them.
- Conventional commits (`feat:`, `test:`, `docs:`). Squash merge. Commit messages in English.
- Reserved mount path prefixes (collide with core `out/` paths): `assets`, `data`, `media`, `api`, `search`, `s`, `admin`, `lobby`, `theme`.
- Source HTML must use **relative** asset paths; absolute paths break under a project deployment base (`/blog/`). Documented, not rewritten.

---

### Task 1: Config schema + validation + path resolution

**Files:**
- Modify: `crates/oxibuilder-core/src/config.rs` (add `MountConfig`, `Config.mounts`, `validate_mounts`, `resolve_mount_sources`, `ConfigError::InvalidMounts`; call from `load`).

**Interfaces:**
- Produces: `pub struct MountConfig { id, source: PathBuf, path, title_ko, title_en, description: Option, icon: Option, open_in_new_tab: bool }`; `Config.mounts: Vec<MountConfig>`; `Config::validate_mounts(&self) -> Result<(), String>`; `Config::resolve_mount_sources(&mut self, base: &Path)`. Tasks 2–4 consume `MountConfig`.

- [ ] **Step 1: Write failing tests** — append to the `mod tests` block in `config.rs`:

```rust
#[test]
fn parses_mounts_section() {
    let cfg = Config::from_toml_str(
        r#"
[site]
name = "S"
base_url = "https://b.dev"

[[mounts]]
id = "portfolio"
source = "../portfolio"
path = "portfolio"
title_ko = "포트폴리오"
title_en = "Portfolio"
description = "Hand-crafted work"
"#,
    )
    .unwrap();
    assert_eq!(cfg.mounts.len(), 1);
    let m = &cfg.mounts[0];
    assert_eq!(m.id, "portfolio");
    assert_eq!(m.path, "portfolio");
    assert_eq!(m.title_en, "Portfolio");
    assert_eq!(m.description.as_deref(), Some("Hand-crafted work"));
    assert!(!m.open_in_new_tab); // default false
}

#[test]
fn validate_rejects_reserved_path_prefix() {
    let mut cfg = Config::default();
    cfg.mounts.push(MountConfig {
        id: "x".into(), source: "/a".into(), path: "assets".into(),
        title_ko: "k".into(), title_en: "e".into(),
        description: None, icon: None, open_in_new_tab: false,
    });
    assert!(cfg.validate_mounts().is_err());
}

#[test]
fn validate_rejects_duplicate_id() {
    let mut cfg = Config::default();
    for _ in 0..2 {
        cfg.mounts.push(MountConfig {
            id: "dup".into(), source: "/a".into(), path: "a".into(),
            title_ko: "k".into(), title_en: "e".into(),
            description: None, icon: None, open_in_new_tab: false,
        });
    }
    let err = cfg.validate_mounts().unwrap_err();
    assert!(err.contains("duplicate mount id"), "{err}");
}

#[test]
fn resolve_mount_sources_makes_relative_absolute() {
    let mut cfg = Config::default();
    cfg.mounts.push(MountConfig {
        id: "p".into(), source: "../portfolio".into(), path: "portfolio".into(),
        title_ko: "k".into(), title_en: "e".into(),
        description: None, icon: None, open_in_new_tab: false,
    });
    let base = std::path::Path::new("/srv/oxibuilder");
    cfg.resolve_mount_sources(base);
    assert_eq!(cfg.mounts[0].source, std::path::PathBuf::from("/srv/oxibuilder/../portfolio"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p oxibuilder-core --lib config::tests`
Expected: FAIL — `MountConfig` / `validate_mounts` / `resolve_mount_sources` do not exist; `cfg.mounts` field missing.

- [ ] **Step 3: Implement** — add to `config.rs`:

```rust
const RESERVED_MOUNT_PATHS: &[&str] = &[
    "assets", "data", "media", "api", "search", "s", "admin", "lobby", "theme",
];

#[derive(Debug, Clone, Deserialize)]
pub struct MountConfig {
    pub id: String,
    pub source: PathBuf,
    pub path: String,
    pub title_ko: String,
    pub title_en: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub open_in_new_tab: bool,
}
```

Add the field to `Config` (next to the other `#[serde(default)]` fields):

```rust
#[serde(default)]
pub mounts: Vec<MountConfig>,
```

Add `mounts: Vec::new()` to both `Config::default()` impls (the `Default` impl and anywhere a `Config` is constructed literally). Add methods + the error variant:

```rust
impl Config {
    /// Structural validation of `[[mounts]]`: unique ids/paths, no reserved
    /// prefixes, no `..`/`.`/absolute paths. Pure (no filesystem access).
    pub fn validate_mounts(&self) -> Result<(), String> {
        let mut ids = std::collections::HashSet::new();
        let mut paths = std::collections::HashSet::new();
        for m in &self.mounts {
            if !ids.insert(&m.id) {
                return Err(format!("duplicate mount id: {}", m.id));
            }
            let norm = m.path.trim_matches('/');
            if norm.is_empty() {
                return Err(format!("mount {} has empty path", m.id));
            }
            if norm
                .split('/')
                .any(|seg| seg == ".." || seg == ".")
            {
                return Err(format!("mount {} has invalid path: {}", m.id, m.path));
            }
            let top = norm.split('/').next().unwrap();
            if RESERVED_MOUNT_PATHS.contains(&top) {
                return Err(format!(
                    "mount {} uses reserved path prefix: {}",
                    m.id, top
                ));
            }
            if !paths.insert(norm) {
                return Err(format!("duplicate mount path: {}", m.path));
            }
        }
        Ok(())
    }

    /// Resolve each mount's `source` to an absolute path relative to `base`.
    /// Warns (non-fatal) when a source dir does not exist.
    pub fn resolve_mount_sources(&mut self, base: &Path) {
        for m in &mut self.mounts {
            if !m.source.is_absolute() {
                m.source = base.join(&m.source);
            }
            if !m.source.is_dir() {
                tracing::warn!(
                    "mount {} source not found: {}",
                    m.id,
                    m.source.display()
                );
            }
        }
    }
}
```

Add the error variant to `ConfigError`:

```rust
#[error("invalid [[mounts]] config: {0}")]
InvalidMounts(String),
```

In `Config::load`, after `apply_env_overrides()` and before `Ok(cfg)`, add:

```rust
cfg.validate_mounts()
    .map_err(ConfigError::InvalidMounts)?;
cfg.resolve_mount_sources(path.parent().unwrap_or_else(|| Path::new(".")));
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p oxibuilder-core --lib config::tests`
Expected: PASS (4 new tests + existing config tests).

- [ ] **Step 5: Commit**

```bash
git add crates/oxibuilder-core/src/config.rs
git commit -m "feat(core): add [[mounts]] config schema with validation and path resolution"
```

---

### Task 2: Build copy mechanism (MountCopy + write_build_output step)

**Files:**
- Modify: `crates/oxibuilder-core/src/builder.rs` (add `MountCopy`, `BuildInputs.mounts`, `MountCopy::from_config`).
- Modify: `crates/oxibuilder-core/src/build_writer.rs` (add the copy step).
- Test: `crates/oxibuilder-core/tests/static_mounts.rs` (new).

**Interfaces:**
- Consumes: `crate::config::MountConfig` (Task 1).
- Produces: `pub struct MountCopy { source: PathBuf, path: String }`; `BuildInputs.mounts: Vec<MountCopy>`; `MountCopy::from_config(&MountConfig) -> MountCopy`. Task 3 consumes `MountCopy::from_config`.

- [ ] **Step 1: Write failing tests** — create `crates/oxibuilder-core/tests/static_mounts.rs`:

```rust
//! Static mount copy behavior + MountConfig→MountCopy mapping.

use oxibuilder_core::build_writer::write_build_output;
use oxibuilder_core::builder::{BuildInputs, BuildOutput, MountCopy, StaticPage};
use oxibuilder_core::config::MountConfig;
use tempfile::TempDir;

fn page(rel: &str, body: &str) -> StaticPage {
    StaticPage { path: rel.to_string(), content: body.to_string() }
}

fn empty_output_with(pages: Vec<StaticPage>) -> BuildOutput {
    BuildOutput { pages, search_docs: vec![], extensions_data: vec![] }
}

#[test]
fn write_build_output_copies_mount_into_out() {
    let tmp = TempDir::with_prefix("oxibuilder-mount-").unwrap();
    let out = tmp.path().join("out");
    let media = tmp.path().join("media");
    std::fs::create_dir_all(&media).unwrap();

    // Mount source: index.html + nested asset.
    let src = tmp.path().join("portfolio");
    std::fs::create_dir_all(src.join("assets")).unwrap();
    std::fs::write(src.join("index.html"), "<!DOCTYPE html><html>portfolio</html>").unwrap();
    std::fs::write(src.join("assets").join("pic.png"), b"PNGBYTES").unwrap();

    let out_struct = empty_output_with(vec![page(
        "index.html",
        "<!DOCTYPE html><html><body>lobby</body></html>",
    )]);
    let mut inputs = BuildInputs::new("https://example.com/", "paper", "seed");
    inputs.mounts = vec![MountCopy { source: src.clone(), path: "portfolio".into() }];
    write_build_output(&out_struct, &out, &media, &inputs).unwrap();

    // Mount materialized under out/portfolio/.
    let html = std::fs::read_to_string(out.join("portfolio").join("index.html")).unwrap();
    assert!(html.contains("portfolio"), "mount index missing: {html}");
    assert!(out.join("portfolio").join("assets").join("pic.png").exists(), "nested asset missing");

    // Core lobby index untouched (still the SPA shell).
    let lobby = std::fs::read_to_string(out.join("index.html")).unwrap();
    assert!(lobby.contains("lobby"), "core index clobbered: {lobby}");
}

#[test]
fn mount_copy_from_config_normalizes_path() {
    let mc = MountConfig {
        id: "p".into(),
        source: "/abs/portfolio".into(),
        path: "/portfolio/".into(),
        title_ko: "k".into(),
        title_en: "e".into(),
        description: None,
        icon: None,
        open_in_new_tab: false,
    };
    let copy = MountCopy::from_config(&mc);
    assert_eq!(copy.path, "portfolio", "leading/trailing slashes stripped");
    assert_eq!(copy.source, std::path::PathBuf::from("/abs/portfolio"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p oxibuilder-core --test static_mounts`
Expected: FAIL — `MountCopy` does not exist; `BuildInputs` has no `mounts` field.

- [ ] **Step 3: Implement** — in `builder.rs`, add the struct next to `StaticPage`:

```rust
/// A resolved static mount to copy into `out/{path}/` at build time.
#[derive(Debug, Clone)]
pub struct MountCopy {
    /// Absolute source directory.
    pub source: std::path::PathBuf,
    /// Normalized URL prefix / `out/` subdirectory (no leading/trailing slash).
    pub path: String,
}

impl MountCopy {
    /// Map a validated, path-resolved `MountConfig` to a build copy spec.
    pub fn from_config(m: &crate::config::MountConfig) -> Self {
        Self {
            source: m.source.clone(),
            path: m.path.trim_matches('/').to_string(),
        }
    }
}
```

Add the field to `BuildInputs` (next to `image_manifest`). `BuildInputs` derives only `Debug, Clone` (not `Serialize`), so no serde attribute is needed:

```rust
/// Configured static mounts (`[[mounts]]`), sources already resolved absolute.
/// Copied verbatim into `out/{path}/` after core assets.
pub mounts: Vec<MountCopy>,
```

And default it in `BuildInputs::new` alongside the other fields:

```rust
mounts: Vec::new(),
```

In `build_writer.rs`, add a new step **after** step 10b (derived images) and **before** step 11 (manifest), inside `write_build_output`:

```rust
// 10c. Copy static mounts (`[[mounts]]`) into out/{path}/. Sources are
//      resolved absolute at config load; missing sources are a hard error
//      here (a mount was configured but its directory is gone).
for mount in &inputs.mounts {
    let dst = out_dir.join(&mount.path);
    copy_dir_recursive(&mount.source, &dst).map_err(|e| {
        let msg = format!(
            "static mount '{}' (from {}): {e}",
            mount.path,
            mount.source.display()
        );
        Box::<dyn std::error::Error + Send + Sync>::from(msg)
    })?;
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p oxibuilder-core --test static_mounts`
Expected: PASS.

- [ ] **Step 5: Run clippy**

Run: `cargo clippy -p oxibuilder-core --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add crates/oxibuilder-core/src/builder.rs crates/oxibuilder-core/src/build_writer.rs crates/oxibuilder-core/tests/static_mounts.rs
git commit -m "feat(core): copy static mounts into out/ during build"
```

---

### Task 3: Wire config.mounts into all build call sites

**Files:**
- Modify: `crates/oxibuilder-cli/src/commands/build.rs` (≈ line 70).
- Modify: `crates/oxibuilder-console/src/build/build_run.rs` (≈ line 113).
- Modify: `crates/oxibuilder-core/src/http.rs` (≈ line 124).

**Interfaces:**
- Consumes: `MountCopy::from_config` + `config.mounts` (Tasks 1–2).
- Produces: every build path (CLI, console background, on-demand) materializes mounts. No new public API.

The copy + mapping logic is unit-tested in Task 2; this task is mechanical wiring verified by compilation + the workspace suite.

- [ ] **Step 1: CLI build** — in `crates/oxibuilder-cli/src/commands/build.rs`, after `let mut inputs = BuildInputs::new(...)` (≈ line 70), add:

```rust
inputs.mounts = config
    .mounts
    .iter()
    .map(oxibuilder_core::builder::MountCopy::from_config)
    .collect();
```

`config` is already in scope here (it is loaded above for `BuildInputs`).

- [ ] **Step 2: Console background build** — in `crates/oxibuilder-console/src/build/build_run.rs`, the build runs inside a `spawn_blocking` closure that already captures `base_url_task` / `theme_task`. Capture the resolved mounts the same way before spawning:

```rust
let mounts_task = state.config.mounts.clone();
```

Then inside the closure, after `let mut inputs = BuildInputs::new(...)` (≈ line 113), add:

```rust
inputs.mounts = mounts_task
    .iter()
    .map(oxibuilder_core::builder::MountCopy::from_config)
    .collect();
```

(Confirm `state.config.mounts` is the resolved `Vec<MountConfig>` — `Config::load` resolves sources at startup.)

- [ ] **Step 3: On-demand build endpoint** — in `crates/oxibuilder-core/src/http.rs`, change `let inputs =` to `let mut inputs =` at the `BuildInputs::new(...)` call (≈ line 124), then add:

```rust
inputs.mounts = config
    .mounts
    .iter()
    .map(crate::builder::MountCopy::from_config)
    .collect();
```

`config` is in scope (used on the preceding line for `config.site.base_url`).

- [ ] **Step 4: Build the whole workspace + run the suite**

Run: `cargo build --workspace && cargo test --workspace`
Expected: build OK; all tests pass (including Task 2's `static_mounts`). If a call-site lacks `config`/`state.config` in scope, capture it the way the neighboring fields (`base_url_task`) are captured.

- [ ] **Step 5: Commit**

```bash
git add crates/oxibuilder-cli/src/commands/build.rs crates/oxibuilder-console/src/build/build_run.rs crates/oxibuilder-core/src/http.rs
git commit -m "feat(build): thread configured mounts through all build paths"
```

---

### Task 4: Manifest mounts

**Files:**
- Modify: `crates/oxibuilder-core/src/manifest.rs` (add `ManifestMount`, `Manifest.mounts`, pure `manifest_mounts` helper, wire into `assemble`; add a test module).

**Interfaces:**
- Consumes: `crate::config::MountConfig` (Task 1).
- Produces: `Manifest.mounts: Vec<ManifestMount>` serialized into `GET /api/console/lobby/manifest` and `data/lobby.json`. Task 5 consumes the same shape in TS.

- [ ] **Step 1: Write failing tests** — add a `#[cfg(test)] mod tests` block at the end of `manifest.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::MountConfig;

    fn mc(id: &str, path: &str, ko: &str, en: &str) -> MountConfig {
        MountConfig {
            id: id.into(),
            source: format!("/srv/{id}").into(),
            path: path.into(),
            title_ko: ko.into(),
            title_en: en.into(),
            description: Some("desc".into()),
            icon: None,
            open_in_new_tab: true,
        }
    }

    #[test]
    fn manifest_mounts_maps_config_fields() {
        let ms = manifest_mounts(&[mc("portfolio", "portfolio", "포트폴리오", "Portfolio")]);
        assert_eq!(ms.len(), 1);
        assert_eq!(ms[0].id, "portfolio");
        assert_eq!(ms[0].display_name.ko, "포트폴리오");
        assert_eq!(ms[0].display_name.en, "Portfolio");
        assert_eq!(ms[0].path, "portfolio");
        assert_eq!(ms[0].description.as_deref(), Some("desc"));
        assert!(ms[0].open_in_new_tab);
    }

    #[test]
    fn manifest_mounts_normalizes_path() {
        let ms = manifest_mounts(&[mc("p", "/stuff/", "k", "e")]);
        assert_eq!(ms[0].path, "stuff");
    }

    #[test]
    fn manifest_mounts_empty_for_no_config() {
        assert!(manifest_mounts(&[]).is_empty());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p oxibuilder-core --lib manifest::tests`
Expected: FAIL — `manifest_mounts` / `ManifestMount` do not exist.

- [ ] **Step 3: Implement** — in `manifest.rs`, add the struct near `ManifestExtension`:

```rust
#[derive(Serialize)]
pub struct ManifestMount {
    pub id: String,
    pub display_name: ManifestLocalized,
    pub path: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub open_in_new_tab: bool,
}
```

Add the field to `Manifest`:

```rust
pub mounts: Vec<ManifestMount>,
```

Add the pure helper:

```rust
/// Map configured static mounts to their manifest representation. Pure
/// (no DB) so it can be unit-tested in isolation; `assemble` calls it.
pub fn manifest_mounts(mounts: &[crate::config::MountConfig]) -> Vec<ManifestMount> {
    mounts
        .iter()
        .map(|m| ManifestMount {
            id: m.id.clone(),
            display_name: ManifestLocalized {
                ko: m.title_ko.clone(),
                en: m.title_en.clone(),
            },
            path: m.path.trim_matches('/').to_string(),
            description: m.description.clone(),
            icon: m.icon.clone(),
            open_in_new_tab: m.open_in_new_tab,
        })
        .collect()
}
```

In `assemble`, populate the new field when constructing the returned `Manifest`:

```rust
mounts: manifest_mounts(&config.mounts),
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p oxibuilder-core --lib manifest::tests`
Expected: PASS.

- [ ] **Step 5: Confirm the full workspace still builds + passes**

Run: `cargo build --workspace && cargo test --workspace`
Expected: OK. (`assemble` now sets `mounts`; all existing manifest consumers compile.)

- [ ] **Step 6: Commit**

```bash
git add crates/oxibuilder-core/src/manifest.rs
git commit -m "feat(core): expose static mounts in the lobby manifest"
```

---

### Task 5: SPA lobby link card

**Files:**
- Modify: `web/src/shared/api.ts` (add `ManifestMount` type, `Manifest.mounts`).
- Modify: `web/src/lobby/Lobby.tsx` (render mount link cards in grid + list modes).

**Interfaces:**
- Consumes: `manifest.mounts` from the API/static JSON (Task 4).
- Produces: clickable lobby cards that perform a full-page navigation to `{path}/`.

- [ ] **Step 1: Extend the manifest types** — in `web/src/shared/api.ts`, add after `ManifestExtension`:

```ts
export interface ManifestMount {
  id: string;
  display_name: LocalizedName;
  path: string;
  description: string | null;
  icon: string | null;
  open_in_new_tab: boolean;
}
```

And extend `Manifest`:

```ts
export interface Manifest {
  site: ManifestSite;
  extensions: ManifestExtension[];
  mounts: ManifestMount[];
}
```

- [ ] **Step 2: Render mount cards** — in `web/src/lobby/Lobby.tsx`:

Update the import to include `ManifestMount`:

```ts
import { fetchManifest, type ManifestExtension, type ManifestMount } from "../shared/api";
```

In the **grid** return (after the `exts.map(...)` block inside `<div className="grid ...">`), append:

```tsx
{manifest.mounts.map((m) => (
  <MountCard key={m.id} mount={m} lang={lang} />
))}
```

In the **list** return (after the `exts.map(...)` rows), append:

```tsx
{manifest.mounts.map((m) => (
  <MountRow key={m.id} mount={m} lang={lang} />
))}
```

Add the two components at the bottom of the file (plain `<a>` — NOT `react-router` `Link` — so the browser does a full page load to the standalone mount HTML):

```tsx
function mountName(m: ManifestMount, lang: Lang): string {
  return (lang === "ko" ? m.display_name.ko : m.display_name.en) ?? m.id;
}

function MountCard({ mount, lang }: { mount: ManifestMount; lang: Lang }) {
  return (
    <a
      href={`${mount.path}/`}
      data-mount={mount.id}
      {...(mount.open_in_new_tab ? { target: "_blank", rel: "noopener" } : {})}
      className={cn(
        "group relative flex flex-col gap-4 rounded-lg border border-line bg-surface p-5 shadow-sm",
        "transition-[transform,box-shadow,border-color] duration-200 ease-out",
        "hover:-translate-y-0.5 hover:shadow-md hover:border-primary/40",
      )}
    >
      <div className="flex size-11 items-center justify-center rounded-md bg-primary/10 text-primary text-xl">
        <span aria-hidden>{mount.icon ?? "🔗"}</span>
      </div>
      <div className="space-y-0.5">
        <h2 className="font-serif text-lg font-semibold leading-tight tracking-tight text-foreground">
          {mountName(mount, lang)}
        </h2>
        {mount.description && <p className="text-sm text-subtle">{mount.description}</p>}
        <p className="text-sm text-subtle">/{mount.path}</p>
      </div>
    </a>
  );
}

function MountRow({ mount, lang }: { mount: ManifestMount; lang: Lang }) {
  return (
    <a
      href={`${mount.path}/`}
      data-mount={mount.id}
      {...(mount.open_in_new_tab ? { target: "_blank", rel: "noopener" } : {})}
      className="group flex items-center gap-3 px-4 py-3 transition-colors hover:bg-canvas"
    >
      <span className="flex size-9 items-center justify-center rounded-md bg-primary/10 text-primary" aria-hidden>
        {mount.icon ?? "🔗"}
      </span>
      <span className="font-serif text-base font-medium text-foreground">
        {mountName(mount, lang)}
      </span>
      <span className="text-sm text-subtle">/{mount.path}</span>
      <span className="ml-auto text-subtle opacity-0 transition-opacity group-hover:opacity-100">→</span>
    </a>
  );
}
```

- [ ] **Step 3: Build the SPA (typecheck + bundle)**

Run: `cd web && bun run build`
Expected: PASS (tsc accepts the new `manifest.mounts` usage; bundle emits). Fix any type errors.

- [ ] **Step 4: Re-embed the SPA into the binary and verify**

Run (from repo root): `cargo build -p oxibuilder-console`
Expected: OK (the `web/dist-static` embed picks up the new bundle).

- [ ] **Step 5: Commit**

```bash
git add web/src/shared/api.ts web/src/lobby/Lobby.tsx
git commit -m "feat(web): render static mount link cards in the lobby"
```

---

### Task 6: Docs — example config, README, changelog

**Files:**
- Modify: `oxibuilder.toml.example` (documented `[[mounts]]` block).
- Modify: `README.md` (one-line mention under Configuration).
- Modify: `CHANGELOG.md` (entry).

- [ ] **Step 1: Example config** — in `oxibuilder.toml.example`, add (commented, under the `[lobby]` section):

```toml
# Static mounts — graft an external static directory (hand-written HTML or the
# build output of any tool) at a URL prefix. Files are copied verbatim into
# out/{path}/ at build time and appear as a link card in the lobby.
# `source` is relative to this config file's directory.
#
# [[mounts]]
# id        = "portfolio"
# source    = "../portfolio"
# path      = "portfolio"          # served at /portfolio/ (under the deploy base)
# title_ko  = "포트폴리오"
# title_en  = "Portfolio"
# description = "Hand-crafted work"
# icon      = "🖼️"
# open_in_new_tab = false
```

- [ ] **Step 2: README** — under `## Configuration`, after the `[lobby]` example, add a short paragraph:

```markdown
**Static mounts** (`[[mounts]]`) graft an external static directory — hand-written
HTML or the build output of any tool (Astro, another SSG) — at a URL prefix
(e.g. `/portfolio/`). Files are copied verbatim into `out/{path}/` at build time
and surface as a link card in the lobby. `source` is relative to the config file.
```

- [ ] **Step 3: Changelog** — add an entry atop `CHANGELOG.md`:

```markdown
## Unreleased
- **Static mounts:** `[[mounts]]` config grafts an external static directory at a
  URL prefix, copied into `out/{path}/` at build time and shown as a lobby link card.
```

- [ ] **Step 4: Verify the example still parses**

Run: `cargo test -p oxibuilder-core --lib config::tests`
Expected: PASS (no behavioral change; the commented block is inert). Optionally confirm the uncommented portion of `oxibuilder.toml.example` parses by eye.

- [ ] **Step 5: Commit**

```bash
git add oxibuilder.toml.example README.md CHANGELOG.md
git commit -m "docs: document static mounts in example config, README, changelog"
```

---

## Final verification

- [ ] `cargo clippy --workspace --all-targets -- -D warnings` — clean.
- [ ] `cargo test --workspace` — all green (139 + new static_mounts/manifest/config tests).
- [ ] `cd web && bun run build` — green.
- [ ] Manual end-to-end: add a `[[mounts]]` entry pointing at a temp dir with an `index.html`, run `oxibuilder build`, confirm `out/<path>/index.html` exists and the lobby JSON (`out/data/lobby.json`) contains the mount; `oxibuilder console --preview` shows the card and it navigates to the mount.

## Self-review notes

- **Spec coverage:** §1 config → Task 1; §2 validation → Task 1; §3 build copy + 3 call sites → Tasks 2–3; §4 manifest → Task 4; §5 SPA card → Task 5; docs → Task 6. §6 (live-console serving) intentionally dropped (spec documents why). Error handling (warn at load / hard error at build) → Tasks 1 & 2.
- **Type consistency:** `MountConfig` (Task 1) consumed identically by `MountCopy::from_config` (Task 2), `manifest_mounts` (Task 4), and the TS `ManifestMount` (Task 5). Field names match across Rust ↔ JSON ↔ TS (`id`, `display_name`, `path`, `description`, `icon`, `open_in_new_tab`).
- **No placeholders:** every code step contains the actual code to write; the only judgment left to the implementer is confirming the exact capture variable name at the `build_run.rs` call site (Step 2 of Task 3), which is described.
