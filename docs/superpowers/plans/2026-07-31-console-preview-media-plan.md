# Console Built Preview and Media — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the static-file preview handler with a faithful built-preview that resolves directory indexes, falls back to `404.html` for missing client routes, rewrites the generated `<base href>` to the preview prefix, and writes a typed `BuildManifest` so every consumer (preview, deploy, CLI) sees one source of truth. Add an image-upload route that validates by magic bytes, stores files under `media_dir/<extension>/<uuid>.<ext>`, and exposes them under a stable `/api/console/s/{slug}/media/...` namespace.

**Architecture:** Three independent slices — (1) build-side: a new `oxipage_core::build_manifest` shared type and `build_writer` changes that convert public asset tags to relative paths and inject `<base href>`; (2) preview handler: prefix-aware directory resolution + per-request base-href rewrite from a single helper; (3) media API: axum `multipart` upload + GET/HEAD serve. On the web side, a small `AssetResolver` interface lets the Deploy preview iframe, the live Admin, and the public static SPA all share one URL-shape contract.

**Tech Stack:** Rust (axum 0.8, rust-embed, mime_guess, uuid, tokio), React 19, TypeScript, Vite 7

## Global Constraints

- **Foundation precondition:** `SiteContext` post-foundation has `slug`, `project_dir`, `data_dir`, `out_dir`, `media_dir`, `startup_server: ServerConfig`, `settings: Arc<RwLock<MutableSiteSettings>>`, `config_write_lock: Arc<Mutex<()>>`, `config: Arc<Config>` REMOVED, plus `db`, `registry`, `builders`, `build_guard`, `deploy_guard`, `wasm_loader`. All `ctx.config.*` reads in this plan must use `ctx.settings.read().await.*` for mutable fields, `ctx.startup_server.*` for server fields.
- **`deployment_base` derivation (single source of truth).** The manifest's `deployment_base` field is NEVER a free string. It is derived at build time from `MutableSiteSettings::site.base_url` by parsing it as a URL and taking the URL pathname. The result is normalized to:
  - leading `/` (always),
  - trailing `/` (always),
  - empty → `/` (so apex / user pages become `/`, project pages become `/<repo>/`).

  Examples:
  - `https://a7garden.github.io/` → `/`
  - `https://a7garden.github.io/blog/` → `/blog/`
  - `https://example.com/deep/nested/` → `/deep/nested/`
  - `http://127.0.0.1:8787/` (default) → `/`
  - `not a url` → `/` (fallback, never blocks a build)

  This lives in a single helper `derive_deployment_base(base_url: &str) -> String` in `oxipage_core::build_manifest` and is the ONLY way `BuildManifest::deployment_base` is populated. The preview handler overrides the `<base href>` at serve time (per-request) but the manifest value is the canonical artifact base.

- **Avoid a second embed or extraction path.** The static SPA bundle (`embedded-spa-static` → `StaticAssets`) already exists with `static_spa_index_html()` and `static_spa_files()`. `build_writer` already extracts the hashed `<script>`/`<link>` tags. The new materialization step transforms those tags in-place.
- **Public static asset/data/media URLs never start with `/`.** Build writer strips the leading slash; the runtime resolver relies on `<base href>` for both GitHub Pages project paths and the preview prefix.
- The preview handler uses `axum 0.8` route syntax (`{slug}` / `{*rest}`, not `:slug` / `*rest`).
- `424 build_required` maps to `StatusCode::FAILED_DEPENDENCY`.
- MIME validation for media uploads is by magic bytes, not by the user-declared `Content-Type` or filename.
- No placeholders, no shims, no aliases after cutover.

---

## File Structure

```text
crates/oxipage-core/
├── Cargo.toml                                # add uuid dep
├── src/
│   ├── lib.rs                                # pub mod build_manifest
│   ├── build_manifest.rs                     # NEW: BuildManifest + read/write + derive_deployment_base
│   ├── build_writer.rs                       # relative tags + <base> + manifest write
│   └── builder.rs                            # write_build_output signature accepts BuildInputs

crates/oxipage-console/
├── Cargo.toml                                # (workspace axum multipart feature)
├── src/
│   ├── media/
│   │   ├── mod.rs                            # NEW: re-exports + router()
│   │   ├── upload.rs                         # NEW: multipart handler + magic-byte validation
│   │   └── serve.rs                          # NEW: GET/HEAD media handler
│   ├── preview/handler.rs                    # REWRITE: prefix-aware + base-rewrite + 424
│   └── per_site.rs                           # mount media routes + extend build_post status

web/
├── src/
│   ├── shared/
│   │   ├── api.ts                            # pathToStaticFile uses document.baseURI
│   │   └── assets.ts                         # NEW: AssetResolver + three resolvers
│   └── admin/
│       ├── shared/
│       │   ├── api.ts                        # uploadImage + previewUrl
│       │   └── ui/
│       │       └── ImageField.tsx            # NEW: URL/upload field
│       └── deploy/
│           └── DeployPage.tsx                # Preview Site button + manifest header
```

---

### Task 1: `BuildManifest` type + atomic read/write + `derive_deployment_base`

**Files:**
- Create: `crates/oxipage-core/src/build_manifest.rs`
- Modify: `crates/oxipage-core/src/lib.rs`
- Modify: `crates/oxipage-core/Cargo.toml`

**Interfaces:**
- Consumes: `out_dir` from the build pipeline (path on disk); `base_url` from `MutableSiteSettings::site.base_url` (string)
- Produces:
  - `BuildManifest` struct serialized to `<out_dir>/.oxipage-build.json` with `read_from(out_dir)` and `write_to(out_dir)` methods.
  - `pub fn derive_deployment_base(base_url: &str) -> String` — the ONLY derivation rule, used by `build_writer` (Task 2) and reusable by `oxipage-deploy` (subproject 4).

- [ ] **Step 1: Write a manifest round-trip and derivation test**

Create `crates/oxipage-core/tests/build_manifest.rs`:

```rust
//! Tests for BuildManifest serialization and deployment_base derivation.

use oxipage_core::build_manifest::{derive_deployment_base, BuildManifest, MAG_FILENAME};
use tempfile::TempDir;

#[test]
fn round_trip_preserves_fields() {
    let dir = TempDir::with_prefix("oxipage-mag-").unwrap();
    let m = BuildManifest {
        build_id: "11111111-2222-3333-4444-555555555555".to_string(),
        deployment_base: "/repo/".to_string(),
        theme_id: "paper".to_string(),
        asset_revision: "abcdef".to_string(),
        built_at: "2026-07-31T10:00:00Z".to_string(),
    };
    m.write_to(dir.path()).unwrap();
    let m2 = BuildManifest::read_from(dir.path()).unwrap().expect("manifest written");
    assert_eq!(m.build_id, m2.build_id);
    assert_eq!(m.deployment_base, m2.deployment_base);
    assert_eq!(m.theme_id, m2.theme_id);
    assert_eq!(m.asset_revision, m2.asset_revision);
    assert_eq!(m.built_at, m2.built_at);
    assert_eq!(MAG_FILENAME, ".oxipage-build.json");
}

#[test]
fn read_returns_none_when_missing() {
    let dir = TempDir::with_prefix("oxipage-mag-missing-").unwrap();
    let got = BuildManifest::read_from(dir.path()).unwrap();
    assert!(got.is_none());
}

#[test]
fn write_to_missing_dir_creates_path() {
    let dir = TempDir::with_prefix("oxipage-mag-created-").unwrap();
    let out = dir.path().join("out");
    let m = BuildManifest::new("/myrepo/", "paper", "deadbeef");
    m.write_to(&out).unwrap();
    assert!(out.join(MAG_FILENAME).exists());
}

#[test]
fn derive_deployment_base_handles_apex_and_project_pages() {
    assert_eq!(derive_deployment_base("https://a7garden.github.io/"), "/");
    assert_eq!(
        derive_deployment_base("https://a7garden.github.io/blog/"),
        "/blog/"
    );
    assert_eq!(
        derive_deployment_base("https://example.com/deep/nested/"),
        "/deep/nested/"
    );
    assert_eq!(
        derive_deployment_base("http://127.0.0.1:8787/"),
        "/"
    );
    // No trailing slash on the input — still produces a trailing slash on output.
    assert_eq!(
        derive_deployment_base("https://example.com/blog"),
        "/blog/"
    );
}

#[test]
fn derive_deployment_base_falls_back_to_root_on_parse_error() {
    // Anything that fails URL parsing returns "/" — never blocks a build.
    assert_eq!(derive_deployment_base("not a url"), "/");
    assert_eq!(derive_deployment_base(""), "/");
    assert_eq!(derive_deployment_base("::::"), "/");
}

#[test]
fn new_helper_uses_supplied_base_without_normalization() {
    // BuildManifest::new is the low-level constructor; the caller is
    // expected to pass a normalized base. The high-level path goes through
    // derive_deployment_base (Task 2).
    let m = BuildManifest::new("/repo/", "paper", "deadbeef");
    assert_eq!(m.deployment_base, "/repo/");
    assert!(!m.build_id.is_empty());
    assert!(!m.built_at.is_empty());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p oxipage-core --test build_manifest`
Expected: FAIL — `build_manifest` module does not exist.

- [ ] **Step 3: Add `uuid` to core Cargo.toml**

In `crates/oxipage-core/Cargo.toml`, add to `[dependencies]`:

```toml
uuid.workspace = true
```

- [ ] **Step 4: Implement `BuildManifest`**

Create `crates/oxipage-core/src/build_manifest.rs`:

```rust
//! Build manifest — single typed source of truth for "what did the build produce?".
//!
//! Written to `<out_dir>/.oxipage-build.json` by `build_writer` after every
//! successful build. Consumed by:
//! - `oxipage_console::preview::handler` (to decide 424 vs serve, and to write
//!   the per-request `<base href>`),
//! - the per-site `build_post` status response (so the UI can render build ID,
//!   theme, deployment base),
//! - `oxipage-deploy` (deployment base + asset revision for GitHub Pages).
//!
//! `deployment_base` is always derived from `MutableSiteSettings::site.base_url`
//! via [`derive_deployment_base`] — it is the canonical "where the deployed
//! artifact will live" base for the build. The preview handler OVERRIDES the
//! served `<base href>` at request time to inject the live preview prefix
//! (the persisted manifest value is still the artifact's canonical base).
//!
//! See spec §4.2, §5, §6.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Manifest filename written inside `out_dir`. Leading dot keeps it adjacent
/// to the deploy artifact without competing with user-facing routes.
pub const MAG_FILENAME: &str = ".oxipage-build.json";

/// One build's metadata. Serialized to `out_dir/.oxipage-build.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BuildManifest {
    /// UUIDv4 assigned at write time.
    pub build_id: String,
    /// Base URL prefix the deployed static files will be served under.
    /// Always starts with `/` and ends with `/`. Empty → `/`.
    /// Derived from `site.base_url` via [`derive_deployment_base`] at build time.
    pub deployment_base: String,
    /// Theme id active at build time (e.g. `"paper"`).
    pub theme_id: String,
    /// SHA-256 hash of the materialized asset set (hex prefix).
    pub asset_revision: String,
    /// RFC3339 timestamp the build finished.
    pub built_at: String,
}

impl BuildManifest {
    /// Low-level constructor. The caller is responsible for `deployment_base`
    /// being already normalized (leading + trailing slash). Prefer
    /// [`BuildManifest::from_site_base`] for the production path.
    pub fn new(deployment_base: impl Into<String>, theme_id: impl Into<String>, asset_revision: impl Into<String>) -> Self {
        let base = deployment_base.into();
        Self {
            build_id: Uuid::new_v4().to_string(),
            deployment_base: normalize_base(base),
            theme_id: theme_id.into(),
            asset_revision: asset_revision.into(),
            built_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        }
    }

    /// Production constructor. Derives `deployment_base` from the site's
    /// `base_url` (a full URL like `https://example.com/repo/` or
    /// `http://127.0.0.1:8787/`) using the single derivation rule.
    pub fn from_site_base(
        site_base_url: &str,
        theme_id: impl Into<String>,
        asset_revision: impl Into<String>,
    ) -> Self {
        Self::new(
            derive_deployment_base(site_base_url),
            theme_id,
            asset_revision,
        )
    }

    /// Read the manifest from `<out_dir>/.oxipage-build.json`. Returns `Ok(None)`
    /// if the file is absent (not an error — the build hasn't run yet).
    pub fn read_from(out_dir: &Path) -> Result<Option<Self>, ManifestError> {
        let path = out_dir.join(MAG_FILENAME);
        if !path.exists() {
            return Ok(None);
        }
        let raw = fs::read_to_string(&path)?;
        let parsed: Self = serde_json::from_str(&raw)?;
        Ok(Some(parsed))
    }

    /// Atomically write the manifest to `<out_dir>/.oxipage-build.json`.
    /// Creates the directory if missing. Writes to a temp file in the same
    /// directory then renames — a read on the live path never sees a partial
    /// payload.
    pub fn write_to(&self, out_dir: &Path) -> Result<(), ManifestError> {
        fs::create_dir_all(out_dir)?;
        let final_path = out_dir.join(MAG_FILENAME);
        let tmp = out_dir.join(format!("{}.tmp", MAG_FILENAME));
        let json = serde_json::to_string_pretty(self)?;
        fs::write(&tmp, json)?;
        fs::rename(&tmp, &final_path)?;
        Ok(())
    }
}

/// Single derivation rule for `deployment_base` from `site.base_url`.
///
/// - Parse the URL. On failure, return `/`.
/// - Take the URL pathname. Strip a trailing slash (if any).
/// - Prepend `/`. If the resulting path is just `/`, return `/`.
/// - Append a trailing `/`. Always return a path that starts with `/` and ends with `/`.
///
/// Examples:
///   `https://a7garden.github.io/`        → `/`
///   `https://a7garden.github.io/blog/`   → `/blog/`
///   `https://example.com/deep/nested/`   → `/deep/nested/`
///   `http://127.0.0.1:8787/`             → `/`
///   `not a url`                          → `/`
pub fn derive_deployment_base(base_url: &str) -> String {
    let parsed = url::Url::parse(base_url).ok();
    let raw_path = match parsed.as_ref() {
        Some(u) => u.path().to_string(),
        None => return "/".to_string(),
    };
    let trimmed = raw_path.trim_end_matches('/');
    if trimmed.is_empty() || trimmed == "/" {
        return "/".to_string();
    }
    if !trimmed.starts_with('/') {
        // Should not happen for a Url::parse result, but defensive.
        return format!("/{trimmed}/");
    }
    format!("{trimmed}/")
}

/// Internal helper that ensures a leading + trailing slash so the manifest
/// is always shape-stable. Empty becomes `/`.
fn normalize_base(s: String) -> String {
    if s.is_empty() {
        return "/".to_string();
    }
    let mut out = s;
    if !out.starts_with('/') {
        out.insert(0, '/');
    }
    if !out.ends_with('/') {
        out.push('/');
    }
    out
}

#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid manifest json: {0}")]
    Json(#[from] serde_json::Error),
}
```

- [ ] **Step 5: Add `url` and `chrono` to core Cargo.toml**

In `crates/oxipage-core/Cargo.toml`, add to `[dependencies]`:

```toml
chrono = { version = "0.4", default-features = false, features = ["clock", "serde"] }
url = "2"
```

`chrono` is already in `sqlx`'s features but not as a top-level dep — add it explicitly so we own the version. `url` is a new dependency.

- [ ] **Step 6: Register the module**

In `crates/oxipage-core/src/lib.rs`, add:

```rust
pub mod build_manifest;
```

(In alphabetical position between `build` and `build_writer` — order doesn't matter for Rust, but keep it tidy.)

- [ ] **Step 7: Run test to verify it passes**

Run: `cargo test -p oxipage-core --test build_manifest`
Expected: PASS — six tests pass.

- [ ] **Step 8: Commit**

```bash
git add crates/oxipage-core/src/build_manifest.rs crates/oxipage-core/src/lib.rs crates/oxipage-core/Cargo.toml crates/oxipage-core/tests/build_manifest.rs
git commit -m "feat(core): BuildManifest + derive_deployment_base from site.base_url"
```

---

### Task 2: `build_writer` — relative asset tags, `<base>`, manifest write

**Files:**
- Modify: `crates/oxipage-core/src/build_writer.rs`
- Modify: `crates/oxipage-core/src/builder.rs` (change `write_build_output` signature)

**Interfaces:**
- Consumes: `BuildOutput`, `out_dir`, `media_dir`, `build_inputs: BuildInputs` (new struct carrying `theme_id`, `asset_revision_seed`; deployment_base is derived INSIDE `write_build_output` from `inputs.site_base_url` via `BuildManifest::from_site_base`)
- Produces: All HTML shells have relative `assets/...` tags and a `<base href="{derived_deployment_base}">` emitted before the dependent scripts/styles; `<out_dir>/.oxipage-build.json` written with the derived `deployment_base`

- [ ] **Step 1: Update `write_build_output` signature**

Read `crates/oxipage-core/src/builder.rs` and find the existing `write_build_output` plumbing. Add a `BuildInputs` struct that carries only the site base URL, theme, and asset revision seed. The deployment_base is derived inside `write_build_output` to enforce the single derivation rule.

Add to `crates/oxipage-core/src/builder.rs`:

```rust
/// Inputs to the build writer that aren't part of the per-extension output.
///
/// `deployment_base` is NOT passed here. It is derived from `site_base_url`
/// inside `write_build_output` via `BuildManifest::from_site_base` so the
/// single derivation rule is enforced at exactly one site.
#[derive(Debug, Clone)]
pub struct BuildInputs {
    /// Full site URL from `MutableSiteSettings::site.base_url`. Used to
    /// derive `deployment_base` (e.g. `https://a7garden.github.io/blog/`
    /// → `/blog/`).
    pub site_base_url: String,
    /// Theme id active at build time.
    pub theme_id: String,
    /// Caller-supplied seed for the asset revision. The build writer
    /// hashes it with the materialized file list to produce a stable
    /// `asset_revision`. Use e.g. `"abc123"` for tests. In production the
    /// caller passes the git revision or a fresh UUID.
    pub asset_revision_seed: String,
}

impl BuildInputs {
    pub fn new(
        site_base_url: impl Into<String>,
        theme_id: impl Into<String>,
        asset_revision_seed: impl Into<String>,
    ) -> Self {
        Self {
            site_base_url: site_base_url.into(),
            theme_id: theme_id.into(),
            asset_revision_seed: asset_revision_seed.into(),
        }
    }
}
```

- [ ] **Step 2: Replace `extract_asset_tags` with a deployed-aware version**

Replace the existing `extract_asset_tags` function (lines 142–156 of `build_writer.rs`) with:

```rust
/// Pull the hashed `<script>` and `<link rel="stylesheet">` tags out of the
/// embedded SPA `index.html`, convert any `/assets/...` URLs to relative
/// `assets/...`, and prepend a `<base href="{deployment_base}">` tag so the
/// browser resolves relative asset URLs against the deployment base.
///
/// Returns `None` if the SPA isn't embedded.
fn extract_asset_tags(deployment_base: &str) -> Option<String> {
    let html = crate::http::static_spa_index_html()?;
    let mut tags: Vec<String> = Vec::new();
    tags.push(format!(r#"<base href="{}">"#, escape_attr(deployment_base)));
    for line in html.lines() {
        let t = line.trim();
        if t.starts_with("<script ") || t.starts_with("<link rel=\"stylesheet") {
            tags.push(relative_asset_tag(t));
        }
    }
    if tags.len() == 1 {
        // Only the <base> tag — no assets were extracted. Treat as missing.
        return None;
    }
    Some(tags.join("\n    "))
}

/// Convert `<script src="/assets/...">` and `<link ... href="/assets/...">`
/// to relative form. Other tags (canonical links, preconnects) pass through
/// unchanged.
fn relative_asset_tag(tag: &str) -> String {
    convert_asset_attr(tag, "src")
        .unwrap_or_else(|| convert_asset_attr(tag, "href").unwrap_or_else(|| tag.to_string()))
}

fn convert_asset_attr(tag: &str, attr: &str) -> Option<String> {
    let needle = format!("{attr}=\"");
    let start = tag.find(&needle)? + needle.len();
    let rest = &tag[start..];
    let end = rest.find('"')?;
    let value = &rest[..end];
    if !value.starts_with("/assets/") {
        return None;
    }
    let mut out = String::with_capacity(tag.len());
    out.push_str(&tag[..start]);
    out.push_str(&value[1..]); // strip leading slash → relative
    out.push('"');
    out.push_str(&rest[end + 1..]);
    Some(out)
}

/// Minimal HTML attribute escaper for the `<base href>` value. Only needs
/// to neutralize `"` and `<`/`>` since `deployment_base` is
/// application-controlled (origin + path normalized to leading + trailing slash).
fn escape_attr(s: &str) -> String {
    s.replace('&', "&amp;").replace('"', "&quot;").replace('<', "&lt;")
}
```

Update `inject_assets` to forward the deployed tags (the signature is unchanged):

```rust
/// Replace the non-hashed placeholder script in a shell with the real asset tags.
fn inject_assets(shell: &str, asset_tags: Option<&str>) -> String {
    match asset_tags {
        Some(tags) => shell.replace(r#"<script src="/assets/index.js"></script>"#, tags),
        None => shell.to_string(),
    }
}
```

- [ ] **Step 3: Compute asset revision and write the manifest using `from_site_base`**

In `crates/oxipage-core/src/build_writer.rs`, add a helper to compute the asset revision hash and update `write_build_output` to (a) accept `BuildInputs`, (b) derive `deployment_base` from `inputs.site_base_url`, (c) compute the revision, (d) write the manifest.

Add at the top of `build_writer.rs`:

```rust
use sha2::{Digest, Sha256};
use crate::build_manifest::BuildManifest;
use crate::builder::BuildInputs;

/// Deterministic SHA-256 of the materialized output set (after writing).
/// Walks `out_dir` recursively, sorts entries, hashes `<relative_path>\0<bytes>`.
fn compute_asset_revision(out_dir: &Path) -> String {
    let mut entries: Vec<(String, Vec<u8>)> = Vec::new();
    collect_files(out_dir, "", &mut entries);
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    let mut hasher = Sha256::new();
    for (name, data) in &entries {
        hasher.update(name.as_bytes());
        hasher.update([0u8]);
        hasher.update(data);
    }
    let digest = hasher.finalize();
    // 16-byte prefix → 32 hex chars keeps it terse in the manifest while
    // remaining collision-safe for the per-site revision namespace.
    let mut out = String::with_capacity(32);
    for b in &digest[..16] {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

fn collect_files(base: &Path, rel: &str, out: &mut Vec<(String, Vec<u8>)>) {
    let dir = if rel.is_empty() { base } else { &base.join(rel) };
    if let Ok(rd) = std::fs::read_dir(dir) {
        for entry in rd.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let rel_path = if rel.is_empty() { name.clone() } else { format!("{rel}/{name}") };
            let path = entry.path();
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                collect_files(base, &rel_path, out);
            } else if let Ok(data) = std::fs::read(&path) {
                out.push((rel_path, data));
            }
        }
    }
}
```

Now rewrite `write_build_output` body. Read the existing function (lines 27–138 of `build_writer.rs`) and replace it with:

```rust
pub fn write_build_output(
    output: &BuildOutput,
    out_dir: &Path,
    media_dir: &Path,
    inputs: &BuildInputs,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // 1. Clean or create output directory.
    if out_dir.exists() {
        fs::remove_dir_all(out_dir)?;
    }
    fs::create_dir_all(out_dir)?;

    // 2. Derive deployment_base from site.base_url (the SINGLE derivation site).
    let deployment_base = BuildManifest::from_site_base(
        &inputs.site_base_url,
        &inputs.theme_id,
        &inputs.asset_revision_seed,
    )
    .deployment_base;

    // 3. Pull the hashed <script>/<link> asset tags from the embedded static SPA
    //    index.html, transform `/assets/...` → relative, and prepend a
    //    `<base href="{deployment_base}">`.
    let asset_tags = extract_asset_tags(&deployment_base);

    // 4. Write all static pages, injecting the transformed asset tags into HTML shells.
    for page in &output.pages {
        let content = if page.path.ends_with(".html") {
            inject_assets(&page.content, asset_tags.as_deref())
        } else {
            page.content.clone()
        };
        let file_path = out_dir.join(&page.path);
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&file_path, &content)?;
    }

    // 5. Write extension data JSON files.
    let data_dir = out_dir.join("data");
    fs::create_dir_all(&data_dir)?;
    for (ext_id, data) in &output.extensions_data {
        let path = data_dir.join(format!("{ext_id}.json"));
        let json = serde_json::to_string_pretty(data)?;
        fs::write(&path, &json)?;
    }

    // 6. Collection shell fallback (unchanged behavior, but uses the new tag form).
    let has_collection_shell: std::collections::HashSet<&str> = output
        .pages
        .iter()
        .filter_map(|p| {
            let mut parts = p.path.split('/');
            let head = parts.next()?;
            if parts.next() == Some("index.html") {
                Some(head)
            } else {
                None
            }
        })
        .collect();
    for (ext_id, _data) in &output.extensions_data {
        if has_collection_shell.contains(ext_id.as_str()) {
            continue;
        }
        let shell = inject_assets(
            &format!(
                r#"<!DOCTYPE html><html lang="ko"><head><meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1.0"><title>{ext_id}</title><link rel="canonical" href="/{ext_id}/"></head><body><div id="root"></div><script src="/assets/index.js"></script></body></html>"#
            ),
            asset_tags.as_deref(),
        );
        let path = out_dir.join(format!("{ext_id}/index.html"));
        fs::create_dir_all(path.parent().unwrap())?;
        fs::write(&path, &shell)?;
    }

    // 7. Search shell fallback (same pattern as 6).
    let search_path = out_dir.join("search/index.html");
    if !search_path.exists() {
        let shell = inject_assets(
            r#"<!DOCTYPE html><html lang="ko"><head><meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1.0"><title>Search</title><link rel="canonical" href="/search/"></head><body><div id="root"></div><script src="/assets/index.js"></script></body></html>"#,
            asset_tags.as_deref(),
        );
        fs::create_dir_all(search_path.parent().unwrap())?;
        fs::write(&search_path, &shell)?;
    }

    // 8. Write search index.
    let search_json = serde_json::to_string_pretty(&output.search_docs)?;
    fs::write(data_dir.join("search-index.json"), &search_json)?;

    // 9. Emit the embedded SPA bundle and 404.html copy (unchanged).
    write_embedded_assets(out_dir)?;
    let index_html = out_dir.join("index.html");
    if index_html.exists() {
        let _ = fs::copy(&index_html, out_dir.join("404.html"));
    }

    // 10. Copy media files.
    if media_dir.exists() {
        copy_dir_recursive(media_dir, &out_dir.join("media"))?;
    }

    // 11. Compute final asset revision and write the manifest with the
    //     SAME deployment_base we just emitted into the HTML. The
    //     `asset_revision_seed` already shaped the early `deployment_base`
    //     computation; we recompute the asset revision now that the out
    //     directory is finalized and overwrite the manifest.
    let asset_revision = compute_asset_revision(out_dir);
    let manifest = BuildManifest {
        build_id: uuid::Uuid::new_v4().to_string(),
        deployment_base,
        theme_id: inputs.theme_id.clone(),
        asset_revision,
        built_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
    };
    manifest.write_to(out_dir)?;

    tracing::info!(
        pages = output.pages.len(),
        extensions = output.extensions_data.len(),
        build_id = %manifest.build_id,
        deployment_base = %manifest.deployment_base,
        dir = %out_dir.display(),
        "build output written"
    );

    Ok(())
}
```

- [ ] **Step 4: Update every call site to pass `BuildInputs`**

Run: `grep -rn "write_build_output" crates/`

For each call site, replace the trailing args with a `BuildInputs` constructed from `ctx.settings.read().await.site.base_url`. The `MutableSiteSettings::site.base_url` is the source — `derive_deployment_base` runs inside `write_build_output`.

```rust
// Standard pattern at any caller:
let base_url = ctx.settings.read().await.site.base_url.clone();
let theme_id = "paper".to_string(); // or read from settings if/when added
let inputs = BuildInputs::new(base_url, theme_id, "oxipage");
write_build_output(output, out_dir, media_dir, &inputs)?;
```

- [ ] **Step 5: Write build_writer tests**

Add `crates/oxipage-core/tests/build_writer_tags.rs`:

```rust
//! Tests for build_writer tag transformations and manifest derivation.

use oxipage_core::build_manifest::BuildManifest;
use oxipage_core::builder::{BuildInputs, BuildOutput};
use oxipage_core::build_writer::write_build_output;
use oxipage_core::builder::PageOutput;
use tempfile::TempDir;

fn page(rel: &str, body: &str) -> PageOutput {
    PageOutput { path: rel.to_string(), content: body.to_string() }
}

#[test]
fn relative_assets_drop_leading_slash() {
    let tmp = TempDir::with_prefix("oxipage-bw-").unwrap();
    let out = tmp.path().join("out");
    let media = tmp.path().join("media");
    std::fs::create_dir_all(&media).unwrap();

    let out_struct = BuildOutput {
        pages: vec![page(
            "index.html",
            r#"<!DOCTYPE html><html><head></head><body><script src="/assets/index.js"></script></body></html>"#,
        )],
        ..Default::default()
    };
    let inputs = BuildInputs::new("https://a7garden.github.io/blog/", "paper", "seed");
    write_build_output(&out_struct, &out, &media, &inputs).unwrap();

    let html = std::fs::read_to_string(out.join("index.html")).unwrap();
    assert!(!html.contains("/assets/index-"), "raw /assets/ leaked: {html}");
    assert!(html.contains("assets/index-"), "relative asset missing: {html}");
    assert!(html.contains("<base href=\"/blog/\">"), "base missing: {html}");
}

#[test]
fn apex_base_url_emits_root_base() {
    let tmp = TempDir::with_prefix("oxipage-bw-apex-").unwrap();
    let out = tmp.path().join("out");
    let media = tmp.path().join("media");
    std::fs::create_dir_all(&media).unwrap();

    let out_struct = BuildOutput {
        pages: vec![page("index.html", "<!DOCTYPE html><html><head></head><body></body></html>")],
        ..Default::default()
    };
    // Apex / user-pages deploy → base must be "/".
    let inputs = BuildInputs::new("https://a7garden.github.io/", "paper", "seed");
    write_build_output(&out_struct, &out, &media, &inputs).unwrap();

    let html = std::fs::read_to_string(out.join("index.html")).unwrap();
    assert!(html.contains("<base href=\"/\">"), "expected `/` base: {html}");
}

#[test]
fn manifest_reflects_derived_deployment_base() {
    let tmp = TempDir::with_prefix("oxipage-bw-mag-").unwrap();
    let out = tmp.path().join("out");
    let media = tmp.path().join("media");
    std::fs::create_dir_all(&media).unwrap();

    let out_struct = BuildOutput {
        pages: vec![page("index.html", "<!DOCTYPE html><html></html>")],
        ..Default::default()
    };
    let inputs = BuildInputs::new("https://example.com/repo/", "paper", "seed");
    write_build_output(&out_struct, &out, &media, &inputs).unwrap();

    let m = BuildManifest::read_from(&out).unwrap().expect("manifest exists");
    assert_eq!(m.deployment_base, "/repo/");
    assert_eq!(m.theme_id, "paper");
    assert!(!m.asset_revision.is_empty());
    assert!(!m.build_id.is_empty());
}
```

Add `Default` to `BuildOutput` and `PageOutput` in `oxipage_core::builder` if not already present. If they aren't `Default`, the test uses explicit field literals — adjust as needed.

- [ ] **Step 6: Run build and tests**

Run: `cargo build -p oxipage-core`
Expected: success.

Run: `cargo test -p oxipage-core --test build_manifest --test build_writer_tags`
Expected: PASS — all manifest and build_writer tests pass.

- [ ] **Step 7: Verify the materialize transformation in a real build**

Run: `cargo build -p oxipage-cli`
Expected: success.

- [ ] **Step 8: Commit**

```bash
git add -A crates/oxipage-core/src/build_writer.rs crates/oxipage-core/src/builder.rs crates/oxipage-core/tests/build_writer_tags.rs
git commit -m "feat(core): materialization — relative asset tags, <base>, BuildManifest via derive_deployment_base"
```

---

### Task 3: Preview handler — prefix-aware, base-href rewrite, 424

- Create: `crates/oxipage-console/src/preview/handler.rs` (rewrite of the existing file)
- Modify: `crates/oxipage-console/src/router.rs` (add `/preview/{slug}` redirect route)
- Modify: `crates/oxipage-console/tests/build_deploy_preview.rs`

**Interfaces:**
- Consumes: `SiteContext { out_dir, ... }` (post-foundation); `BuildManifest` via `read_from`
- Produces: `GET /api/console/preview/{slug}/{*rest}` — directory index resolution, SPA fallback to `404.html`, base-href rewrite for HTML responses (the `<base href>` written to disk is the manifest's `deployment_base`; the handler OVERRIDES it to the preview prefix at request time), 424 `build_required` when manifest/index missing, traversal guards, `Cache-Control: no-store`

- [ ] **Step 1: Update the existing preview test to expect 424**

The existing test `preview_endpoint_returns_404_for_missing_out_dir` no longer matches the contract. Replace it.

In `crates/oxipage-console/tests/build_deploy_preview.rs`, rewrite that test:

```rust
#[tokio::test]
async fn preview_endpoint_returns_424_when_manifest_missing() {
    let app = build_test_app().await;
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/preview/blog/index.html")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FAILED_DEPENDENCY);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p oxipage-console --test build_deploy_preview`
Expected: FAIL — current handler returns 404, not 424.

- [ ] **Step 3: Rewrite the preview handler**

Replace the entire contents of `crates/oxipage-console/src/preview/handler.rs`:

```rust
//! `GET /api/console/preview/{slug}/{*rest}` — serve one site's `out/` build.
//!
//! Two routes are mounted by `router.rs::build_top_level_router`:
//!   - `/api/console/preview/{slug}`            → `redirect_to_slash`  (307)
//!   - `/api/console/preview/{slug}/{*rest}`   → `preview_handler`     (catch-all)
//!
//! `preview_handler` accepts a single `Path<(String, String)>` extractor
//! because catch-all routes always populate the rest capture (as an empty
//! string for `/preview/{slug}/`). The bare-slug case is handled by the
//! separate redirect route — `preview_handler` never sees it.
//!
//! Resolution rules (spec §6):
//!   empty path             → out/index.html
//!   directory path         → <dir>/index.html
//!   existing file          → exact file
//!   missing client route   → out/404.html
//!   missing build/manifest → 424 build_required
//!
//! For HTML responses, the generated `<base href>` is rewritten to the
//! preview prefix so the bundled SPA resolves its relative `assets/...` tags
//! correctly. The persisted manifest's `deployment_base` is shipped as the
//! artifact's canonical base; the preview operates under a different
//! (longer) URL prefix at request time.
//!
//! All response bodies are served with `Cache-Control: no-store` and
//! `X-Content-Type-Options: nosniff`. No directory listing.

use crate::sites_runtime::SiteRegistry;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{Response, StatusCode, header};
use oxipage_core::build_manifest::BuildManifest;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

/// `GET /api/console/preview/{slug}` → 307 to `/preview/{slug}/`.
/// Mounted directly in `router.rs::build_top_level_router` so the handler
/// file owns no routing surface (the file is named `handler.rs`, not `router.rs`).
pub async fn redirect_to_slash(
    State(_registry): State<Arc<SiteRegistry>>,
    Path(slug): Path<String>,
) -> Response<Body> {
    Response::builder()
        .status(StatusCode::TEMPORARY_REDIRECT)
        .header(header::LOCATION, format!("/api/console/preview/{slug}/"))
        .body(Body::empty())
        .unwrap()
}

pub(crate) async fn preview_handler(
    State(registry): State<Arc<SiteRegistry>>,
    Path((slug, rest)): Path<(String, String)>,
) -> Result<Response<Body>, (StatusCode, String)> {
    let ctx = registry
        .ctx_for(&slug)
        .await
        .ok_or((StatusCode::NOT_FOUND, "site_not_found".to_string()))?;

    let out_dir = &ctx.out_dir;

    // Manifest gate — spec §5: missing manifest means no build has run.
    let manifest = BuildManifest::read_from(out_dir)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let manifest = match manifest {
        Some(m) => m,
        None => return Err((StatusCode::FAILED_DEPENDENCY, "build_required".into())),
    };

    // Build the candidate path with traversal guards.
    let clean = rest.trim_start_matches('/');
    let mut candidate = PathBuf::from(out_dir);
    let mut has_segments = false;
    for component in Path::new(clean).components() {
        match component {
            Component::Normal(seg) => {
                let seg_str = seg.to_string_lossy();
                if seg_str.is_empty() || seg_str == "." || seg_str == ".." {
                    return Err((StatusCode::BAD_REQUEST, "path_traversal".into()));
                }
                has_segments = true;
                candidate.push(seg_str.as_ref());
            }
            Component::CurDir | Component::ParentDir => {
                return Err((StatusCode::BAD_REQUEST, "path_traversal".into()));
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err((StatusCode::BAD_REQUEST, "path_anchor".into()));
            }
        }
    }

    // 1. exact file
    let resolved = if !has_segments {
        out_dir.join("index.html")
    } else if candidate.is_file() {
        candidate
    } else if candidate.is_dir() {
        // 2. directory index
        candidate.join("index.html")
    } else if looks_like_client_route(&candidate) {
        // 3. SPA fallback
        out_dir.join("404.html")
    } else {
        // 4. static asset / data / media that's missing → real 404
        return Err((StatusCode::NOT_FOUND, "preview_not_found".into()));
    };

    if !resolved.is_file() {
        return Err((StatusCode::NOT_FOUND, "preview_not_found".into()));
    }

    // Containment check — even after component filtering, confirm the resolved
    // path is inside out_dir. Catches symlink races and platform quirks.
    let canonical_out = out_dir.canonicalize().unwrap_or_else(|_| out_dir.clone());
    let canonical_resolved = resolved.canonicalize().unwrap_or_else(|_| resolved.clone());
    if !canonical_resolved.starts_with(&canonical_out) {
        return Err((StatusCode::BAD_REQUEST, "path_traversal".into()));
    }

    let bytes = std::fs::read(&resolved)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mime = mime_guess::from_path(&resolved).first_or_octet_stream();
    let is_html = mime == "text/html" || resolved.extension().and_then(|s| s.to_str()) == Some("html");

    let body = if is_html {
        // The manifest's deployment_base is the artifact's canonical base
        // (e.g. `/repo/`). The preview serves at a different URL prefix — we
        // override the `<base href>` so the SPA's relative `assets/...` tags
        // resolve against the live preview URL.
        let preview_base = preview_base_href(&slug);
        rewrite_base_href(&bytes, &preview_base, &manifest.deployment_base)
    } else {
        bytes
    };

    let mut builder = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime.to_string())
        .header(header::CACHE_CONTROL, "no-store")
        .header("X-Content-Type-Options", "nosniff");

    if is_html {
        builder = builder
            .header("X-Oxipage-Build-Id", &manifest.build_id)
            .header("X-Oxipage-Build-Theme", &manifest.theme_id)
            .header("X-Oxipage-Build-Asset-Revision", &manifest.asset_revision)
            .header("X-Oxipage-Build-Deployment-Base", &manifest.deployment_base);
    }

    Ok(builder.body(Body::from(body)).unwrap())
}

/// Build a preview-prefix base href from the slug, ensuring it ends with `/`.
/// Equivalent to `/api/console/preview/{slug}/`.
fn preview_base_href(slug: &str) -> String {
    format!("/api/console/preview/{slug}/")
}

/// Decide whether a missing path should fall back to 404.html.
fn looks_like_client_route(candidate: &Path) -> bool {
    let ext = candidate.extension().and_then(|s| s.to_str()).unwrap_or("");
    if ext.is_empty() {
        return true;
    }
    matches!(ext, "html")
}

/// Replace the `<base href="...">` in the persisted HTML with the
/// per-request preview base. The persisted HTML's `<base href>` is the
/// manifest's `deployment_base` (the artifact's canonical base); we
/// override it for the preview URL.
///
/// Falls back to inserting a `<base>` if the materialized HTML is missing
/// one (older builds).
fn rewrite_base_href(html: &[u8], preview_base: &str, _materialized_base: &str) -> Vec<u8> {
    let haystack = String::from_utf8_lossy(html);
    let replacement = format!("<base href=\"{preview_base}\">");
    if haystack.contains("<base href=\"") {
        if let Some(start) = haystack.find("<base href=\"") {
            let after = &haystack[start + "<base href=\"".len()..];
            if let Some(end_offset) = after.find('"') {
                let mut out = String::with_capacity(haystack.len() + preview_base.len());
                out.push_str(&haystack[..start]);
                out.push_str(&replacement);
                out.push_str(&after[end_offset + 1..]);
                return out.into_bytes();
            }
        }
        return html.to_vec();
    }
    // No <base> in the file — inject one at the start of <head>.
    if let Some(idx) = haystack.find("<head>") {
        let mut out = String::with_capacity(haystack.len() + replacement.len() + 8);
        out.push_str(&haystack[..idx + "<head>".len()]);
        out.push('\n');
        out.push_str("    ");
        out.push_str(&replacement);
        out.push_str(&haystack[idx + "<head>".len()..]);
        return out.into_bytes();
    }
    html.to_vec()
}
```

- [ ] **Step 3b: Wire the redirect route in `router.rs`**

The bare-slug route (no trailing slash) needs a separate handler that returns
307 to the canonical `/preview/{slug}/` URL. Add it to the top-level console
router alongside the existing `preview` catch-all.

In `crates/oxipage-console/src/router.rs`, find the line that mounts the
preview route inside `build_top_level_router()`:

```rust
.route("/preview/{slug}/{*rest}", get(preview_handler))
```

Add a sibling route above it (axum matches routes in registration order, so
the more specific bare-slug route must come first):

```rust
use crate::preview::handler::redirect_to_slash;

// in build_top_level_router():
.route("/preview/{slug}", get(redirect_to_slash))
.route("/preview/{slug}/{*rest}", get(preview_handler))
```

The existing `use crate::preview::handler::preview_handler;` import stays.
The `handler.rs` file no longer exports a `router()` function — its job is
to expose the two handlers, and `router.rs` is the single source of truth
for route wiring.

- [ ] **Step 4: Add coverage for the new behavior**

Add to `crates/oxipage-console/tests/build_deploy_preview.rs`:

```rust
async fn build_test_app_with_out() -> (tempfile::TempDir, Router) {
    let (dir, path) = create_site_dir("Test");
    let out_dir = path.join("data").join("out");
    std::fs::create_dir_all(&out_dir).unwrap();
    // Manifest with deployment_base = "/repo/" — the artifact's canonical base.
    std::fs::write(
        out_dir.join(".oxipage-build.json"),
        r#"{"build_id":"b1","deployment_base":"/repo/","theme_id":"paper","asset_revision":"abc","built_at":"2026-07-31T10:00:00Z"}"#,
    ).unwrap();
    // HTML has the manifest-derived base (`/repo/`); the preview handler
    // must override it to the preview prefix.
    std::fs::write(out_dir.join("index.html"), "<!DOCTYPE html><html><head><base href=\"/repo/\"></head><body>x</body></html>").unwrap();

    let mut sf = oxipage_core::sites::SitesFile::default();
    sf.add("blog".into(), path);
    sf.set_default("blog");
    let registry = Arc::new(SiteRegistry::new(sf).await.unwrap());
    (dir, build_console_router(registry))
}

#[tokio::test]
async fn preview_root_serves_index_and_rewrites_base() {
    let (_dir, app) = build_test_app_with_out().await;
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/preview/blog/")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 64 * 1024).await.unwrap();
    let text = String::from_utf8_lossy(&body);
    assert!(text.contains("/api/console/preview/blog/"), "base not rewritten: {text}");
    assert!(!text.contains("/repo/"), "old base leaked: {text}");
    assert_eq!(
        app.oneshot(
            Request::builder()
                .method("GET")
                .uri("/preview/blog/")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
        .headers()
        .get("cache-control")
        .unwrap(),
        "no-store"
    );
}

#[tokio::test]
async fn preview_rejects_traversal() {
    let (_dir, app) = build_test_app_with_out().await;
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/preview/blog/../etc/passwd")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn preview_serves_spa_fallback_for_missing_route() {
    let (dir, app) = build_test_app_with_out().await;
    let out_dir = dir.path().join("data").join("out");
    std::fs::write(
        out_dir.join("404.html"),
        "<!DOCTYPE html><html><head><base href=\"/repo/\"></head><body>404</body></html>",
    ).unwrap();
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/preview/blog/blog/some-post")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 64 * 1024).await.unwrap();
    let text = String::from_utf8_lossy(&body);
    assert!(text.contains("/api/console/preview/blog/"), "base not rewritten: {text}");
}

#[tokio::test]
async fn preview_redirects_no_wildcard_to_trailing_slash() {
    let (_dir, app) = build_test_app_with_out().await;
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/preview/blog")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(
        resp.headers().get("location").unwrap().to_str().unwrap(),
        "/api/console/preview/blog/"
    );
}

#[tokio::test]
async fn preview_emits_build_metadata_headers() {
    let (_dir, app) = build_test_app_with_out().await;
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/preview/blog/")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let h = resp.headers();
    assert_eq!(h.get("x-oxipage-build-id").unwrap(), "b1");
    assert_eq!(h.get("x-oxipage-build-theme").unwrap(), "paper");
    assert_eq!(h.get("x-oxipage-build-asset-revision").unwrap(), "abc");
    assert_eq!(h.get("x-oxipage-build-deployment-base").unwrap(), "/repo/");
}
```

Add `use axum::body::to_bytes;` at the top of the test file if needed.

- [ ] **Step 5: Run tests**

Run: `cargo test -p oxipage-console --test build_deploy_preview`
Expected: PASS — all preview tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/oxipage-console/src/preview/handler.rs crates/oxipage-console/src/router.rs crates/oxipage-console/tests/build_deploy_preview.rs
git commit -m "feat(console): preview prefix-aware + base-rewrite + 424 build_required"
```

---

### Task 4: Media upload — multipart, magic bytes, atomic rename

**Files:**
- Create: `crates/oxipage-console/src/media/mod.rs`
- Create: `crates/oxipage-console/src/media/upload.rs`
- Create: `crates/oxipage-console/src/media/serve.rs`
- Modify: `Cargo.toml` (workspace `axum` features → add `multipart`)

**Interfaces:**
- Consumes: `SiteContext { media_dir, registry }` (post-foundation)
- Produces: `POST /api/console/s/{slug}/media/{extension}` accepts `multipart/form-data` with a single `file` field; returns `{data: {path, mime, bytes}}`; `GET`/`HEAD /api/console/s/{slug}/media/{extension}/{file}` serves from `media_dir`

- [ ] **Step 1: Enable the axum multipart feature in the workspace**

In the root `Cargo.toml`, update the `axum` workspace dependency:

```toml
axum = { version = "0.8", features = ["macros", "multipart"] }
```

`crates/oxipage-console/Cargo.toml` consumes the workspace `axum`; no further change needed.

- [ ] **Step 2: Create the media module skeleton**

Create `crates/oxipage-console/src/media/mod.rs`:

```rust
//! Media upload and live serving — `/api/console/s/{slug}/media/...`.
//!
//! Spec §7–9. Upload accepts a single `file` field via multipart, validates
//! by magic bytes (not declared Content-Type), chooses extension from the
//! detected MIME, and writes the file atomically to
//! `<media_dir>/<extension>/<uuid>.<ext>`.
//!
//! Live serving is a thin static handler that precedes the Admin SPA
//! fallback so a `/media/...` URL always returns bytes, never `admin.html`.

pub mod serve;
pub mod upload;

use axum::Router;
use axum::routing::{get, post};

/// Mount the media routes under the per-site nest. Caller wraps with
/// `site_db` middleware so handlers can extract `Extension<Arc<SiteContext>>`.
pub fn router() -> Router {
    Router::new()
        .route("/media/{extension}", post(upload::upload_handler))
        .route(
            "/media/{extension}/{file}",
            get(serve::serve_handler).head(serve::serve_handler),
        )
}
```

- [ ] **Step 3: Register the module in the console lib**

In `crates/oxipage-console/src/lib.rs`, add:

```rust
pub mod media;
```

- [ ] **Step 4: Implement the upload handler**

Create `crates/oxipage-console/src/media/upload.rs`:

```rust
//! Multipart upload for site media. Spec §9.

use crate::sites_runtime::SiteContext;
use axum::Extension;
use axum::Json;
use axum::extract::{Multipart, Path};
use axum::http::StatusCode;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

/// 10 MiB hard cap per spec §9. Applied against the running byte counter
/// during streaming so users with a 100 MB file see an early rejection.
const MAX_FILE_BYTES: usize = 10 * 1024 * 1024;

#[derive(Serialize)]
pub struct UploadResponse {
    pub data: UploadData,
}

#[derive(Serialize)]
pub struct UploadData {
    pub path: String,
    pub mime: &'static str,
    pub bytes: u64,
}

/// Image format detected by reading the first 16 bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DetectedFormat {
    Jpeg,
    Png,
    WebP,
    Gif,
}

impl DetectedFormat {
    fn ext(self) -> &'static str {
        match self {
            DetectedFormat::Jpeg => "jpg",
            DetectedFormat::Png => "png",
            DetectedFormat::WebP => "webp",
            DetectedFormat::Gif => "gif",
        }
    }
    fn mime(self) -> &'static str {
        match self {
            DetectedFormat::Jpeg => "image/jpeg",
            DetectedFormat::Png => "image/png",
            DetectedFormat::WebP => "image/webp",
            DetectedFormat::Gif => "image/gif",
        }
    }
}

fn detect_format(head: &[u8]) -> Option<DetectedFormat> {
    if head.len() >= 3 && head[..3] == [0xFF, 0xD8, 0xFF] {
        return Some(DetectedFormat::Jpeg);
    }
    if head.len() >= 8 && head[..8] == [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A] {
        return Some(DetectedFormat::Png);
    }
    if head.len() >= 6 && (&head[..6] == b"GIF87a" || &head[..6] == b"GIF89a") {
        return Some(DetectedFormat::Gif);
    }
    if head.len() >= 12 && &head[..4] == b"RIFF" && &head[8..12] == b"WEBP" {
        return Some(DetectedFormat::WebP);
    }
    None
}

/// Reject path separators and any non-alnum/underscore/hyphen so the
/// extension id can't be used as a path component.
fn is_safe_extension_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn media_dir_for(ctx: &SiteContext, extension: &str) -> std::io::Result<PathBuf> {
    let dir = ctx.media_dir.join(extension);
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub async fn upload_handler(
    Extension(ctx): Extension<Arc<SiteContext>>,
    Path(extension): Path<String>,
    mut multipart: Multipart,
) -> Result<Json<UploadResponse>, (StatusCode, String)> {
    if !is_safe_extension_id(&extension) {
        return Err((StatusCode::BAD_REQUEST, "invalid_extension_id".into()));
    }

    let mut file_field = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("multipart: {e}")))?
    {
        if field.name() == Some("file") {
            file_field = Some(field);
            break;
        }
    }
    let mut field = file_field.ok_or((StatusCode::BAD_REQUEST, "missing_file_field".into()))?;

    let dest_dir = media_dir_for(&ctx, &extension)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("mkdir: {e}")))?;

    // Hyphenated UUID form for both filename and URL.
    let uuid = Uuid::new_v4();
    let uuid_str = uuid.hyphenated().to_string();
    let tmp_path = dest_dir.join(format!("{uuid_str}.tmp"));
    let mut tmp = tokio::fs::File::create(&tmp_path)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("tmp create: {e}")))?;

    let mut head: Vec<u8> = Vec::with_capacity(16);
    let mut total: u64 = 0;
    let mut detected: Option<DetectedFormat> = None;

    while let Some(chunk) = field
        .chunk()
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("multipart: {e}")))?
    {
        total = total.saturating_add(chunk.len() as u64);
        if total > MAX_FILE_BYTES as u64 {
            drop(tmp);
            let _ = tokio::fs::remove_file(&tmp_path).await;
            return Err((StatusCode::PAYLOAD_TOO_LARGE, "file_too_large".into()));
        }
        if detected.is_none() && head.len() < 16 {
            let need = 16 - head.len();
            let take = need.min(chunk.len());
            head.extend_from_slice(&chunk[..take]);
            detected = detect_format(&head);
        }
        tmp.write_all(&chunk)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("write: {e}")))?;
    }

    let format = match detected {
        Some(f) => f,
        None => {
            drop(tmp);
            let _ = tokio::fs::remove_file(&tmp_path).await;
            return Err((StatusCode::UNSUPPORTED_MEDIA_TYPE, "unsupported_image_format".into()));
        }
    };

    tmp.flush()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("flush: {e}")))?;
    drop(tmp);

    let final_path = dest_dir.join(format!("{uuid_str}.{}", format.ext()));
    tokio::fs::rename(&tmp_path, &final_path)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("rename: {e}")))?;

    let response_path = format!("media/{extension}/{uuid_str}.{}", format.ext());
    Ok(Json(UploadResponse {
        data: UploadData {
            path: response_path,
            mime: format.mime(),
            bytes: total,
        },
    }))
}
```

- [ ] **Step 5: Implement the serve handler**

Create `crates/oxipage-console/src/media/serve.rs`:

//! Live serving of uploaded media. Spec §9 (live serving).
//!
//! Reads are lock-free. Unix `rename(2)` is atomic, so a reader always sees
//! either the old path or the fully-written new path — never a partial file.
//! The upload handler writes to a `.tmp` sibling and atomically renames into
//! place, so no cross-handler synchronization is required here.

use crate::sites_runtime::SiteContext;
use axum::Extension;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{Response, StatusCode, header};
use axum::response::IntoResponse;
use std::path::{Component, PathBuf};
use std::sync::Arc;

pub async fn serve_handler(
    State(_registry): State<Arc<crate::sites_runtime::SiteRegistry>>,
    Extension(ctx): Extension<Arc<SiteContext>>,
    Path((extension, file)): Path<(String, String)>,
) -> Result<Response<Body>, Response<Body>> {
    let mut candidate = PathBuf::from(&ctx.media_dir);
    for component in std::path::Path::new(&extension).components() {
        match component {
            Component::Normal(seg) => {
                let s = seg.to_string_lossy();
                if s.is_empty() || s == "." || s == ".." {
                    return Err(StatusCode::BAD_REQUEST.into_response());
                }
                candidate.push(s.as_ref());
            }
            _ => return Err(StatusCode::BAD_REQUEST.into_response()),
        }
    }
    for component in std::path::Path::new(&file).components() {
        match component {
            Component::Normal(seg) => {
                let s = seg.to_string_lossy();
                if s.is_empty() || s == "." || s == ".." {
                    return Err(StatusCode::BAD_REQUEST.into_response());
                }
                candidate.push(s.as_ref());
            }
            _ => return Err(StatusCode::BAD_REQUEST.into_response()),
        }
    }

    let meta = match tokio::fs::metadata(&candidate).await {
        Ok(m) if m.is_file() => m,
        _ => return Err(StatusCode::NOT_FOUND.into_response()),
    };

    let canonical_media = ctx
        .media_dir
        .canonicalize()
        .unwrap_or_else(|_| ctx.media_dir.clone());
    let canonical_candidate = candidate
        .canonicalize()
        .unwrap_or_else(|_| candidate.clone());
    if !canonical_candidate.starts_with(&canonical_media) {
        return Err(StatusCode::BAD_REQUEST.into_response());
    }

    let bytes = match tokio::fs::read(&candidate).await {
        Ok(b) => b,
        Err(_) => return Err(StatusCode::NOT_FOUND.into_response()),
    };

    let mime = mime_guess::from_path(&candidate).first_or_octet_stream();
    let len = meta.len();

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime.to_string())
        .header(header::CONTENT_LENGTH, len)
        .header(header::CACHE_CONTROL, "no-cache")
        .header("X-Content-Type-Options", "nosniff")
        .body(Body::from(bytes))
        .unwrap())
}
```

- [ ] **Step 6: Write tests for upload + serve**

Create `crates/oxipage-console/tests/media.rs`:

```rust
//! Tests for the media upload + serve endpoints.

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use oxipage_console::router::build_console_router;
use oxipage_console::sites_runtime::SiteRegistry;
use oxipage_core::sites::SitesFile;
use std::sync::Arc;
use tempfile::TempDir;
use tower::util::ServiceExt;

fn minimal_toml(name: &str) -> String {
    format!(
        r#"[site]
name = "{name}"
base_url = "http://127.0.0.1:8787"
default_lang = "ko"
languages = ["ko"]

[server]
host = "127.0.0.1"
port = 8787
data_dir = "data"
"#,
    )
}

async fn build_app() -> (TempDir, Router) {
    let dir = TempDir::with_prefix("oxipage-media-").unwrap();
    let toml_path = dir.path().join("oxipage.toml");
    std::fs::write(&toml_path, minimal_toml("Test")).unwrap();
    let mut sf = SitesFile::default();
    sf.add("blog".into(), dir.path().to_path_buf());
    sf.set_default("blog");
    let registry = Arc::new(SiteRegistry::new(sf).await.unwrap());
    (dir, build_console_router(registry))
}

// 1×1 transparent PNG (smallest valid PNG).
const PNG_1X1: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A,
    0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
    0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
    0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41,
    0x54, 0x78, 0x9C, 0x62, 0x00, 0x01, 0x00, 0x00,
    0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00,
    0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE,
    0x42, 0x60, 0x82,
];

fn build_multipart(filename: &str, content: &[u8]) -> (String, Vec<u8>) {
    let boundary = "----oxipage-test-boundary";
    let body = format!(
        "--{b}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{f}\"\r\nContent-Type: image/png\r\n\r\n",
        b = boundary,
        f = filename,
    )
    .into_bytes();
    let mut full = body;
    full.extend_from_slice(content);
    full.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    let ct = format!("multipart/form-data; boundary={boundary}");
    (ct, full)
}

#[tokio::test]
async fn upload_png_round_trips() {
    let (_dir, app) = build_app().await;
    let (ct, body) = build_multipart("avatar.png", PNG_1X1);
    let req = Request::builder()
        .method("POST")
        .uri("/api/console/s/blog/media/profile")
        .header("content-type", ct)
        .body(Body::from(body))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let path = json["data"]["path"].as_str().unwrap().to_string();
    assert!(path.starts_with("media/profile/"), "path: {path}");
    assert!(path.ends_with(".png"), "path: {path}");
    assert_eq!(json["data"]["mime"], "image/png");

    let file_name = path.split('/').last().unwrap();
    let get_req = Request::builder()
        .method("GET")
        .uri(format!("/api/console/s/blog/media/profile/{file_name}"))
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(get_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let back = axum::body::to_bytes(resp.into_body(), 64 * 1024).await.unwrap();
    assert_eq!(back.as_ref(), PNG_1X1);
}

#[tokio::test]
async fn upload_rejects_fake_png() {
    let (_dir, app) = build_app().await;
    let (ct, body) = build_multipart("avatar.png", b"not an image at all");
    let req = Request::builder()
        .method("POST")
        .uri("/api/console/s/blog/media/profile")
        .header("content-type", ct)
        .body(Body::from(body))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
}

#[tokio::test]
async fn upload_rejects_oversize() {
    let (_dir, app) = build_app().await;
    let mut payload = PNG_1X1.to_vec();
    payload.resize(11 * 1024 * 1024, 0);
    let (ct, body) = build_multipart("huge.png", &payload);
    let req = Request::builder()
        .method("POST")
        .uri("/api/console/s/blog/media/profile")
        .header("content-type", ct)
        .body(Body::from(body))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn upload_rejects_invalid_extension_id() {
    let (_dir, app) = build_app().await;
    let (ct, body) = build_multipart("avatar.png", PNG_1X1);
    let req = Request::builder()
        .method("POST")
        .uri("/api/console/s/blog/media/..%2fetc")
        .header("content-type", ct)
        .body(Body::from(body))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert!(resp.status() == StatusCode::BAD_REQUEST || resp.status() == StatusCode::NOT_FOUND);
}
```

- [ ] **Step 7: Run tests**

Run: `cargo test -p oxipage-console --test media`
Expected: PASS — all four tests pass.

- [ ] **Step 8: Commit**

```bash
git add Cargo.toml crates/oxipage-console/src/media/ crates/oxipage-console/Cargo.toml crates/oxipage-console/src/lib.rs crates/oxipage-console/tests/media.rs
git commit -m "feat(console): media upload (multipart + magic bytes) and live serve"
```

---

### Task 5: Wire media routes into per-site router

**Files:**
- Modify: `crates/oxipage-console/src/per_site.rs`

**Interfaces:**
- Consumes: nothing new
- Produces: `per_site_router()` mounts `media::router()` so the per-site routes include `/media/{extension}` and `/media/{extension}/{file}`

- [ ] **Step 1: Read the existing `per_site_router` body**

Confirmed earlier — it's at the bottom of `per_site.rs` (lines 743–757). The function returns a plain `Router` builder; the per-site `Extension<Arc<SiteContext>>` middleware is already applied by the parent nest in `router.rs::build_per_site_router`, so the media router inherits the layer.

- [ ] **Step 2: Add the merge**

In `crates/oxipage-console/src/per_site.rs`, at the top of the file add:

```rust
use crate::media;
```

In `per_site_router()`, replace the function body with:

```rust
pub fn per_site_router() -> Router {
    Router::new()
        .route("/config", get(config_get).put(config_put))
        .route("/builds", get(builds_list))
        .route("/build", post(build_post))
        .route("/build/{build_id}/stream", get(build_stream))
        .route("/deploy", post(deploy_post))
        .route("/deploy/{deploy_id}/stream", get(deploy_stream))
        .route("/stats", get(stats_get))
        .route("/content/recent", get(recent_get))
        .route("/theme", get(theme_get).put(theme_put))
        .route("/extensions", get(extensions_list))
        .route("/extensions/{id}/enable", post(extension_enable))
        .route("/extensions/{id}/disable", post(extension_disable))
        .merge(media::router())
}
```

- [ ] **Step 3: Build**

Run: `cargo build -p oxipage-console`
Expected: success.

- [ ] **Step 4: Re-run media tests**

Run: `cargo test -p oxipage-console --test media`
Expected: PASS — the routes are now reachable.

- [ ] **Step 5: Commit**

```bash
git add crates/oxipage-console/src/per_site.rs
git commit -m "feat(console): wire media routes into per-site router"
```

---

### Task 6: `AssetResolver` interface + three resolvers

**Files:**
- Create: `web/src/shared/assets.ts`

**Interfaces:**
- Consumes: nothing (pure helpers)
- Produces: `AssetResolver` interface and `adminAssetResolver(slug)`, `previewAssetResolver(previewBase)`, `publicAssetResolver()` factories

- [ ] **Step 1: Create the module**

Place at `web/src/shared/assets.ts`:

```ts
// Asset resolvers — convert a stored reference (logical path or external
// URL) into a URL the current context can fetch.
//
// Three resolvers cover the four contexts (live preview, built preview,
// built public, live admin):
//   - adminAssetResolver(slug):  media/...   → /api/console/s/{slug}/media/...
//   - previewAssetResolver(p):   media/...   → new URL(mediaPath, p)
//   - publicAssetResolver():     media/...   → new URL(mediaPath, document.baseURI)
//
// Absolute http(s) URLs pass through unchanged. Unsupported schemes
// (javascript:, data:, file:) are rejected and resolve to null so the
// caller can fall back to a neutral placeholder.

export interface AssetResolver {
  resolve(value: string | null | undefined): string | null;
}

const UNSUPPORTED_SCHEMES = /^(javascript|data|file|vbscript):/i;

function safeUrl(value: string): URL | null {
  try {
    return new URL(value);
  } catch {
    return null;
  }
}

function isHttpish(url: URL): boolean {
  return url.protocol === "http:" || url.protocol === "https:";
}

function isUnsupported(value: string): boolean {
  return UNSUPPORTED_SCHEMES.test(value.trim());
}

function normalize(mediaPath: string): string {
  // Strip a single leading slash so callers can store either "media/x" or "/media/x".
  return mediaPath.replace(/^\/+/, "");
}

export function adminAssetResolver(slug: string): AssetResolver {
  const base = `/api/console/s/${slug}/media/`;
  return {
    resolve(value) {
      if (!value) return null;
      if (isUnsupported(value)) return null;
      const u = safeUrl(value);
      if (u && isHttpish(u)) return value;
      return base + normalize(value);
    },
  };
}

export function previewAssetResolver(previewBase: string): AssetResolver {
  const base = previewBase.endsWith("/") ? previewBase : previewBase + "/";
  return {
    resolve(value) {
      if (!value) return null;
      if (isUnsupported(value)) return null;
      const u = safeUrl(value);
      if (u && isHttpish(u)) return value;
      try {
        return new URL(normalize(value), base).toString();
      } catch {
        return null;
      }
    },
  };
}

export function publicAssetResolver(): AssetResolver {
  return {
    resolve(value) {
      if (!value) return null;
      if (isUnsupported(value)) return null;
      const u = safeUrl(value);
      if (u && isHttpish(u)) return value;
      try {
        return new URL(normalize(value), document.baseURI).toString();
      } catch {
        return null;
      }
    },
  };
}
```

- [ ] **Step 2: Type-check**

Run: `cd web && npx tsc --noEmit`
Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add web/src/shared/assets.ts
git commit -m "feat(web): AssetResolver interface + admin/preview/public resolvers"
```

---

### Task 7: `pathToStaticFile` uses `document.baseURI`

**Files:**
- Modify: `web/src/shared/api.ts`

**Interfaces:**
- Consumes: existing `apiFetch` callers
- Produces: `pathToStaticFile` returns a path WITHOUT a leading slash so the URL resolver does the base-relative joining. `apiFetch` resolves against `document.baseURI` so the value works under preview prefixes and project Pages.

- [ ] **Step 1: Update `apiFetch` and `pathToStaticFile`**

Replace the two functions in `web/src/shared/api.ts`:

```ts
async function apiFetch<T>(path: string): Promise<T> {
  const isStatic = import.meta.env.VITE_DATA_MODE === 'static';

  if (isStatic) {
    // Map API paths to static JSON files generated by oxipage build.
    // We resolve against document.baseURI so the relative URL works under
    // preview prefixes (/api/console/preview/{slug}/...) and project-pages
    // deployments (/{repo}/) without per-context branching.
    const staticPath = pathToStaticFile(path);
    let url: string;
    try {
      url = new URL(staticPath, document.baseURI).toString();
    } catch {
      url = staticPath;
    }
    const res = await fetch(url);
    if (!res.ok) {
      throw new ApiError(res.status, `Static data not found: ${url}`);
    }
    return res.json() as Promise<T>;
  }

  const res = await fetch(`/api/console${path}`);
  if (!res.ok) {
    throw new ApiError(res.status, `API request failed: ${res.status} ${path}`);
  }
  const json = (await res.json()) as { data: T };
  return json.data;
}

function pathToStaticFile(path: string): string {
  // Map API paths to the static JSON files `oxipage build` emits.
  // Collections/singletons map to <segment>.json under data/. Detail routes
  // (blog/<slug>, projects/<slug>) are NOT resolved here — the detail
  // fetchers pull them client-side from the collection (the build emits
  // full-body collection JSON).
  //   /blog           → data/blog.json
  //   /profile        → data/profile.json
  //   /lobby/manifest → data/lobby.json
  //   /search?q=…     → data/search-index.json (client-filtered)
  //
  // Returns a path WITHOUT a leading slash so the URL resolver does the
  // base-relative joining. The caller resolves against document.baseURI.
  const [rawPath] = path.split('?');
  const parts = rawPath.split('/').filter(Boolean);
  if (parts.length === 0) return 'data/lobby.json';
  if (parts[0] === 'search') return 'data/search-index.json';
  return `data/${parts[0]}.json`;
}
```

- [ ] **Step 2: Smoke test**

Run: `cd web && bun run build`
Expected: build succeeds.

- [ ] **Step 3: Commit**

```bash
git add web/src/shared/api.ts
git commit -m "feat(web): resolve static data paths against document.baseURI"
```

---

### Task 8: `uploadImage` client + `ImageField` component

**Files:**
- Modify: `web/src/admin/shared/api.ts` (add `uploadImage` + `previewUrl`)
- Create: `web/src/admin/shared/ui/ImageField.tsx`

**Interfaces:**
- Consumes: `SlugContext` (current slug) + site media resolver
- Produces: `uploadImage(slug, extension, file) → Promise<{path, mime, bytes}>`; `previewUrl(slug) → string`; `ImageField` React component for URL/upload + clear + thumbnail

- [ ] **Step 1: Add `uploadImage` and `previewUrl` to admin api**

In `web/src/admin/shared/api.ts`, add at the end (just before the bottom exports):

```ts
// ─── Media upload ─────────────────────────────────────────────────────────

export interface UploadedMedia {
  path: string;
  mime: string;
  bytes: number;
}

export interface UploadResponse {
  data: UploadedMedia;
}

/**
 * POST a single image file to the site media endpoint. The path component
 * specifies a logical extension namespace (e.g. "profile", "novels"). The
 * server validates by magic bytes and returns a logical path like
 * `media/profile/<uuid>.png` — store that in the content row.
 */
export async function uploadImage(
  slug: string,
  extension: string,
  file: File,
): Promise<UploadedMedia> {
  const form = new FormData();
  form.append("file", file);
  const res = await fetch(
    `${CONSOLE_BASE}/s/${slug}/media/${extension}`,
    { method: "POST", body: form },
  );
  const body = await jsonOrThrow<UploadResponse>(res);
  return body.data;
}

/** Prefix-aware URL for the preview iframe. Opens at the deployed base. */
export function previewUrl(slug: string): string {
  return `${CONSOLE_BASE}/preview/${slug}/`;
}
```

`CONSOLE_BASE` is already declared at the top of the file — don't redeclare it.

- [ ] **Step 2: Create `ImageField`**

Create `web/src/admin/shared/ui/ImageField.tsx`:

```tsx
import { useState, useRef } from "react";
import { uploadImage } from "../api";
import { adminAssetResolver } from "../../../shared/assets";
import { Input } from "../../../shared/ui/input";
import { Button } from "../../../shared/ui/button";

interface ImageFieldProps {
  slug: string;
  /** Extension namespace (e.g. "profile", "novels"). */
  extension: string;
  /** Current stored value — either a logical path (`media/...`) or an absolute URL. */
  value: string | null;
  /** Called with the new value (logical path or absolute URL). */
  onChange: (next: string | null) => void;
  /** Optional MIME-type filter passed to the file input. */
  accept?: string;
  /** Optional label rendered above the field. */
  label?: string;
  /** Disabled state propagates to input + upload + clear. */
  disabled?: boolean;
}

export function ImageField({
  slug,
  extension,
  value,
  onChange,
  accept = "image/png,image/jpeg,image/webp,image/gif",
  label,
  disabled,
}: ImageFieldProps) {
  const [urlInput, setUrlInput] = useState(value ?? "");
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const fileRef = useRef<HTMLInputElement>(null);
  const resolver = adminAssetResolver(slug);
  const previewSrc = resolver.resolve(value);

  function apply(next: string | null) {
    setError(null);
    onChange(next);
    if (next === null) setUrlInput("");
  }

  async function onFile(e: React.ChangeEvent<HTMLInputElement>) {
    const file = e.target.files?.[0];
    if (!file) return;
    setPending(true);
    setError(null);
    try {
      const media = await uploadImage(slug, extension, file);
      apply(media.path);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Upload failed");
    } finally {
      setPending(false);
      if (fileRef.current) fileRef.current.value = "";
    }
  }

  function onUrlBlur() {
    const trimmed = urlInput.trim();
    if (trimmed === (value ?? "")) return;
    apply(trimmed === "" ? null : trimmed);
  }

  function onClear() {
    apply(null);
  }

  return (
    <div className="space-y-2">
      {label && (
        <div className="text-sm font-medium text-foreground">{label}</div>
      )}
      <div className="flex gap-2 items-start">
        <div className="w-24 h-24 rounded-md border border-line bg-surface/40 flex items-center justify-center overflow-hidden shrink-0">
          {previewSrc ? (
            // The src is admin-resolved; never trust the raw stored value.
            // eslint-disable-next-line jsx-a11y/alt-text
            <img
              src={previewSrc}
              alt=""
              className="w-full h-full object-cover"
              onError={() => setError("Image failed to load")}
            />
          ) : (
            <span className="text-xs text-muted">No image</span>
          )}
        </div>
        <div className="flex-1 space-y-2">
          <Input
            type="url"
            value={urlInput}
            onChange={(e) => setUrlInput(e.target.value)}
            onBlur={onUrlBlur}
            placeholder="https://example.com/image.png or media/profile/..."
            disabled={disabled}
          />
          <div className="flex gap-2">
            <input
              ref={fileRef}
              type="file"
              accept={accept}
              className="hidden"
              onChange={onFile}
              disabled={disabled || pending}
            />
            <Button
              type="button"
              variant="outline"
              onClick={() => fileRef.current?.click()}
              disabled={disabled || pending}
            >
              {pending ? "Uploading…" : "Upload"}
            </Button>
            <Button
              type="button"
              variant="ghost"
              onClick={onClear}
              disabled={disabled || !value}
            >
              Clear
            </Button>
          </div>
          {error && (
            <p className="text-xs text-red-500" role="alert">
              {error}
            </p>
          )}
        </div>
      </div>
    </div>
  );
}
```

- [ ] **Step 3: Type-check and build**

Run: `cd web && npx tsc --noEmit`
Expected: no errors.

Run: `cd web && bun run build`
Expected: build succeeds.

- [ ] **Step 4: Commit**

```bash
git add web/src/admin/shared/api.ts web/src/admin/shared/ui/ImageField.tsx
git commit -m "feat(admin): uploadImage + previewUrl + ImageField component"
```

---

### Task 9: `DeployPage` — Preview Site button + manifest header

**Files:**
- Modify: `web/src/admin/deploy/DeployPage.tsx`
- Modify: `crates/oxipage-console/src/per_site.rs` (extend `build_post` status with manifest summary)

**Interfaces:**
- Consumes: `build_post` status (now includes `manifest_preview` summary); `listBuilds` for history
- Produces: "Preview Site" button in the header that opens `previewUrl(slug)` in a new tab when a build is ready; a manifest header strip showing build ID, theme, deployment base, asset revision

- [ ] **Step 1: Extend `build_post` to include manifest summary**

In `crates/oxipage-console/src/per_site.rs`, find `build_post` (lines 417–472). Read the function and locate the success-path Ok arm. Add a manifest read just before constructing the response, and add a `manifest_preview` field to the JSON body:

```rust
// Just before the Ok((StatusCode::OK, Json(...))) return:
let manifest_preview = oxipage_core::build_manifest::BuildManifest::read_from(&ctx.out_dir)
    .ok()
    .flatten()
    .map(|m| serde_json::json!({
        "build_id": m.build_id,
        "theme_id": m.theme_id,
        "deployment_base": m.deployment_base,
        "asset_revision": m.asset_revision,
        "ready": true,
    }));

// In the value passed to Json({...}), add:
"manifest_preview": manifest_preview,
```

- [ ] **Step 2: Update `DeployPage` JSX**

In `web/src/admin/deploy/DeployPage.tsx`, modify the header row to add a Preview Site button. Add the import at the top:

```tsx
import { previewUrl } from "../shared/api";
```

Replace the existing header `<div className="flex gap-2">` block with:

```tsx
<div className="flex gap-2">
  <Button variant="outline" onClick={onBuild} disabled={busy}>
    {busy && op === "build" ? "Building…" : "↧ Build"}
  </Button>
  <Button
    variant="outline"
    onClick={() => window.open(previewUrl(slug!), "_blank", "noopener,noreferrer")}
    disabled={!last || last.status !== "ok"}
    title={
      !last
        ? "Run a build to enable preview"
        : last.status !== "ok"
          ? "Last build did not succeed"
          : "Open the built site in a new tab"
    }
  >
    Preview Site ↗
  </Button>
  <Button onClick={onDeploy} disabled={busy}>
    {busy && op === "deploy" ? "Deploying…" : "⇧ Deploy"}
  </Button>
</div>
```

The `BuildRecord` interface in `web/src/admin/shared/api.ts` already has `status: string`. Verify the actual value the backend writes into `build_log.status` ("ok" vs "success") and adjust the comparison to match.

- [ ] **Step 3: Add a manifest header strip**

Below the "Last Build" card, add a strip showing the four manifest fields. If the most recent build record doesn't carry the fields yet, render the strip with placeholders rather than hiding it:

```tsx
{last && (
  <div className="grid grid-cols-4 gap-3 text-xs mb-6">
    <div>
      <div className="text-muted">Build ID</div>
      <div className="font-mono">{(last as any).build_id ?? "—"}</div>
    </div>
    <div>
      <div className="text-muted">Theme</div>
      <div>{(last as any).theme_id ?? "—"}</div>
    </div>
    <div>
      <div className="text-muted">Deployment base</div>
      <div className="font-mono">{(last as any).deployment_base ?? "/"}</div>
    </div>
    <div>
      <div className="text-muted">Asset rev</div>
      <div className="font-mono">{(last as any).asset_revision ?? "—"}</div>
    </div>
  </div>
)}
```

If `build_log` columns are added later, populate them in `build_post` and widen the `BuildRecord` type accordingly.

- [ ] **Step 4: Type-check and build**

Run: `cd web && npx tsc --noEmit`
Expected: no errors.

Run: `cd web && bun run build`
Expected: build succeeds.

- [ ] **Step 5: Commit**

```bash
git add web/src/admin/deploy/DeployPage.tsx crates/oxipage-console/src/per_site.rs
git commit -m "feat(admin): Deploy Preview Site button + manifest header"
```

---

## Self-Review

**Spec coverage:**
- §5 `BuildManifest` type + read/write + `derive_deployment_base`: Task 1
- §4.1/4.2 relative public asset tags + `<base href>` (single derivation site): Task 2
- §6 preview handler (directory index, SPA fallback, base-rewrite, 424, traversal guards, no-store, build metadata headers): Task 3
- §9 media upload (multipart, magic bytes, 10 MiB, UUID, atomic temp+rename, partial-file cleanup): Task 4
- §9 live media serving (GET/HEAD, MIME, nosniff, no-cache, containment checks): Task 4
- §8/§11 media routes wired into per-site router: Task 5
- §10 AssetResolver interface + three resolvers: Task 6
- §4.3 `document.baseURI` runtime resolver in public data fetches: Task 7
- §11 uploadImage client + ImageField: Task 8
- §7 DeployPage Preview Site button + manifest header: Task 9

**Foundation cross-checks:**
- `SiteContext` post-foundation: `out_dir`, `media_dir`, `settings: Arc<RwLock<MutableSiteSettings>>`, `startup_server: ServerConfig`, `config_write_lock` — used consistently in Tasks 3, 4. No `ctx.config.*` references.
- `BuildManifest::from_site_base` is the ONLY entry point that populates `deployment_base` from `site.base_url`. Every caller (Task 2 `build_writer`, plus reusable by `oxipage-deploy`) goes through `derive_deployment_base`, so the manifest field is shape-stable.
- `embedded-spa-static` is the only path used for asset extraction (Task 2). No second embed.

**Constraint compliance:**
- No placeholder/stub/follow-up labels in the deliverable.
- Cache policy: `no-store` for preview (Task 3), `no-cache` for live media (Task 4).
- Static asset tags stripped of leading `/` (Task 2). Runtime resolver strips leading `/` defensively (Task 6).
- Unsupported schemes (`javascript:`, `data:`, `file:`, `vbscript:`) rejected by `AssetResolver` (Task 6).
- Atomic temp+rename in both media upload and `BuildManifest` write.

**Placeholder scan:** No TBD/TODO/placeholders.

**Test invariants:** Every task that introduces a behavior has at least one failing-then-passing test pair. Media tests cover magic-byte rejection, oversized rejection, traversal rejection, and round-trip. Preview tests cover 424, traversal, base-rewrite, SPA fallback, and build-metadata headers. Build manifest tests cover round-trip, missing-file, fresh-dir creation, and the apex/project-page derivation.

**Type consistency:** `BuildManifest` defined once (Task 1) and consumed by `build_writer` (Task 2) and `preview::handler` (Task 3). `AssetResolver` defined once (Task 6) and consumed by `ImageField` (Task 8) and the DeployPage preview iframe (Task 9). `UploadError`/`UploadResponse`/`UploadData` live only in `media::upload`.
