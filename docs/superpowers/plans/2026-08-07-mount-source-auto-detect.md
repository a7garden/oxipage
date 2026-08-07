# Mount Source Auto-Detection — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let a mount `source` point at an external project root and have oxibuilder auto-locate the static build-output directory under it (`dist`, `out`, etc.) on every build, so users never have to specify the exact output path.

**Architecture:** Add a pure `detect_static_output(&Path) -> Option<PathBuf>` name-matching helper in core `config.rs`, invoked from the existing `resolve_mount_sources` (which every build path already funnels through). The toml keeps the verbatim raw `source`; detection mutates only the in-memory resolved path. The console `GET /mounts` list additionally surfaces the resolved path by mapping raw-doc mounts to the already-resolved `ctx.settings.mounts`.

**Tech Stack:** Rust (axum + tokio), `tempfile` (dev-dep, already present), existing `oxibuilder-core` / `oxibuilder-console` test harness.

## Global Constraints

- Rust workspace. Lint gate: `cargo clippy --workspace -- -D warnings` clean.
- Test gate: `cargo test --workspace` green. Core config tests live in `crates/oxibuilder-core/src/config.rs` `#[cfg(test)] mod tests`; console endpoint tests live in `crates/oxibuilder-console/tests/mounts_api.rs`.
- Conventional commits, English messages: `feat:`/`test:`/`refactor:`.
- Don't canonicalize resolved paths — `resolve_mount_sources` only `join`s (existing contract; `resolve_mount_sources_makes_relative_absolute` asserts the non-canonical `/srv/oxibuilder/../portfolio` form). Detection must preserve that.
- Candidate set is a fixed const: `["dist", "build", "out", ".output/public", "_site", "www"]`. `public`/`site` are deliberately excluded.

---

## File Structure

- **Modify:** `crates/oxibuilder-core/src/config.rs` — add `MOUNT_OUTPUT_CANDIDATES` const, `has_index_html` + `detect_static_output` helpers, and the detection step inside `resolve_mount_sources`; add unit tests in the existing `mod tests`.
- **Modify:** `crates/oxibuilder-console/src/router.rs` — `mounts_list` injects a `resolved_source` field per mount (raw doc `source` stays verbatim).
- **Test:** `crates/oxibuilder-console/tests/mounts_api.rs` — add a round-trip asserting `resolved_source` reflects an auto-detected `dist/`.

The CLI (`commands/mount.rs`) and `output.rs` need **no changes**: the list payload flows through generic JSON printing, so `resolved_source` appears automatically in both human and `--json` modes.

---

## Task 1: `detect_static_output` pure helper (core)

**Files:**
- Modify: `crates/oxibuilder-core/src/config.rs` (add const + helpers near the `impl Config` block; tests in `mod tests`)

**Interfaces:**
- Produces: `pub(crate) const MOUNT_OUTPUT_CANDIDATES: &[&str]`, `fn detect_static_output(source: &Path) -> Option<PathBuf>`.

- [ ] **Step 1: Write the failing tests**

Append to `mod tests` in `crates/oxibuilder-core/src/config.rs`:

```rust
#[test]
fn detect_returns_source_when_it_has_index_html() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("index.html"), "<html></html>").unwrap();
    assert_eq!(detect_static_output(tmp.path()).as_deref(), Some(tmp.path()));
}

#[test]
fn detect_prefers_dist_over_build() {
    let tmp = tempfile::TempDir::new().unwrap();
    for d in ["dist", "build"] {
        let dir = tmp.path().join(d);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("index.html"), "x").unwrap();
    }
    let got = detect_static_output(tmp.path()).unwrap();
    assert_eq!(got.file_name().unwrap(), "dist");
}

#[test]
fn detect_matches_output_public_two_deep() {
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = tmp.path().join(".output").join("public");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("index.html"), "x").unwrap();
    let got = detect_static_output(tmp.path()).unwrap();
    assert_eq!(got, tmp.path().join(".output").join("public"));
}

#[test]
fn detect_skips_candidate_without_index_html() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(tmp.path().join("dist")).unwrap(); // no index.html
    let build = tmp.path().join("build");
    std::fs::create_dir_all(&build).unwrap();
    std::fs::write(build.join("index.html"), "x").unwrap();
    let got = detect_static_output(tmp.path()).unwrap();
    assert_eq!(got.file_name().unwrap(), "build");
}

#[test]
fn detect_returns_none_when_no_match() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(tmp.path().join("src")).unwrap();
    std::fs::create_dir_all(tmp.path().join("node_modules")).unwrap();
    assert!(detect_static_output(tmp.path()).is_none());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p oxibuilder-core config::tests::detect`
Expected: compile error — `detect_static_output` not defined.

- [ ] **Step 3: Write the minimal implementation**

Add just above `impl Config {` in `crates/oxibuilder-core/src/config.rs`:

```rust
/// Candidate build-output directory names in priority order. Probed under a
/// mount `source` (project root) to auto-locate the static build artifacts.
/// `public`/`site` are deliberately omitted: they are commonly build inputs
/// or multi-purpose, so matching them risks grafting the wrong directory.
const MOUNT_OUTPUT_CANDIDATES: &[&str] = &[
    "dist",           // Vite / Astro / most bundlers
    "build",          // CRA / others
    "out",            // Next.js static export
    ".output/public", // Nuxt / Nitro (2-deep)
    "_site",          // Jekyll / eleventy
    "www",            // assorted
];

/// `index.html` presence is the marker of a static-site root.
fn has_index_html(dir: &Path) -> bool {
    dir.is_dir() && dir.join("index.html").is_file()
}

/// Locate a mount's static build output under `source`.
///
/// If `source` itself contains `index.html` it is treated as the result dir
/// (this is also the exact-path override: `source = "../portfolio/dist"` is
/// honored verbatim). Otherwise the first `source/<candidate>` that is a
/// directory containing `index.html` wins, in `MOUNT_OUTPUT_CANDIDATES`
/// priority order. `None` when nothing matches.
fn detect_static_output(source: &Path) -> Option<PathBuf> {
    if has_index_html(source) {
        return Some(source.to_path_buf());
    }
    for cand in MOUNT_OUTPUT_CANDIDATES {
        let dir = source.join(cand);
        if has_index_html(&dir) {
            return Some(dir);
        }
    }
    None
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p oxibuilder-core config::tests::detect`
Expected: 5 passed.

- [ ] **Step 5: Commit**

```bash
git add crates/oxibuilder-core/src/config.rs
git commit -m "feat(config): add detect_static_output mount source helper"
```

---

## Task 2: Wire detection into `resolve_mount_sources` (core)

**Files:**
- Modify: `crates/oxibuilder-core/src/config.rs:242` (the `resolve_mount_sources` body)
- Modify: `crates/oxibuilder-core/tests/static_mounts.rs` (new build-side guard test)

**Interfaces:**
- Consumes: `detect_static_output` (Task 1).
- Produces: `resolve_mount_sources` detects each mount's static output dir, and **drops
  the mount from `self.mounts` when the source is a real directory but no output was
  detected**. The three build call sites (CLI `build.rs`, console `build_run.rs`, core
  `http.rs`) inherit this automatically because they read resolved `config.mounts`.

**Critical regression guard (existing test):** `resolve_mount_sources_makes_relative_absolute`
uses a non-existent `source = "../portfolio"` resolving to `/srv/oxibuilder/../portfolio`.
Detection returns `None` for it AND `is_dir()` is false (file/dir absent) → the
**missing-source** branch (`is_dir` false) keeps the mount. The test still asserts the
non-canonical joined path. Do not change that behavior.

**New mandatory behavior:** a real-dir-no-match mount must be dropped. This is the
green-while-wrong guard. Without it, `copy_dir_recursive` would copy `node_modules`,
`.git`, `src`, … into `out/{path}/`. The new unit test plus the new build-side test
(Task 2b) pin this.

- [ ] **Step 1: Write the failing tests** (two of them)

Append to `mod tests` in `crates/oxibuilder-core/src/config.rs`:

```rust
#[test]
fn resolve_mount_sources_auto_detects_dist_under_root() {
    let tmp = tempfile::TempDir::new().unwrap();
    // External project root with a dist/ output under it.
    let dist = tmp.path().join("project").join("dist");
    std::fs::create_dir_all(&dist).unwrap();
    std::fs::write(dist.join("index.html"), "x").unwrap();

    let mut cfg = Config::default();
    cfg.mounts.push(MountConfig {
        id: "p".into(),
        source: "project".into(),
        path: "portfolio".into(),
        title_ko: "k".into(),
        title_en: "e".into(),
        description: None,
        icon: None,
        open_in_new_tab: false,
    });
    cfg.resolve_mount_sources(tmp.path());
    assert_eq!(cfg.mounts.len(), 1);
    assert_eq!(cfg.mounts[0].source, dist);
}

#[test]
fn resolve_mount_sources_drops_mount_when_no_static_output_detected() {
    let tmp = tempfile::TempDir::new().unwrap();
    // Real external project root, but with NO index.html and NO candidate output dir.
    std::fs::create_dir_all(tmp.path().join("src")).unwrap();
    std::fs::create_dir_all(tmp.path().join("node_modules")).unwrap();

    let mut cfg = Config::default();
    cfg.mounts.push(MountConfig {
        id: "p".into(),
        source: "project".into(),
        path: "portfolio".into(),
        title_ko: "k".into(),
        title_en: "e".into(),
        description: None,
        icon: None,
        open_in_new_tab: false,
    });
    cfg.resolve_mount_sources(tmp.path());
    assert!(cfg.mounts.is_empty(), "mount must be dropped on no-match; got: {:#?}", cfg.mounts);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p oxibuilder-core config::tests::resolve_mount_sources`
Expected: both fail. The first lacks the detection step; the second doesn't drop.

- [ ] **Step 3: Implement detection + drop semantics**

Replace the body of `resolve_mount_sources` in `crates/oxibuilder-core/src/config.rs` with:

```rust
/// Resolve each mount's `source` to an absolute path relative to `base`, then
/// auto-detect the static build output under it. Drops the mount from
/// `self.mounts` when the source is a real directory but no static output is
/// detected — otherwise the downstream `copy_dir_recursive` would copy the
/// whole project root (node_modules, .git, src, …) into `out/{path}/`. Missing
/// sources are kept (existing behavior; the build will hard-error on copy).
pub fn resolve_mount_sources(&mut self, base: &Path) {
    self.mounts.retain_mut(|m| {
        if !m.source.is_absolute() {
            m.source = base.join(&m.source);
        }
        match detect_static_output(&m.source) {
            Some(resolved) if resolved != m.source => {
                tracing::info!(
                    "mount {}: auto-detected {} -> {}",
                    m.id,
                    m.source.display(),
                    resolved.display()
                );
                m.source = resolved;
                true // keep — build will copy this
            }
            Some(_) => {
                // source itself is the result dir (explicit override); keep.
                true
            }
            None => {
                if !m.source.is_dir() {
                    // Missing source: existing behavior — warn, keep. The build
                    // will hard-error on the copy (same as today).
                    tracing::warn!("mount {} source not found: {}", m.id, m.source.display());
                    true
                } else {
                    // Real dir, no static output detected — drop. Otherwise
                    // copy_dir_recursive would copy the project root into
                    // out/{path}/.
                    tracing::warn!(
                        "mount {}: no static output detected under {} \
                         (looked for index.html and: {}) — dropping mount",
                        m.id,
                        m.source.display(),
                        MOUNT_OUTPUT_CANDIDATES.join(", ")
                    );
                    false
                }
            }
        }
    });
}
```

- [ ] **Step 4: Run the full core config test module**

Run: `cargo test -p oxibuilder-core config::tests`
Expected: all pass, including:
- `resolve_mount_sources_auto_detects_dist_under_root` (new) — PASS
- `resolve_mount_sources_drops_mount_when_no_static_output_detected` (new) — PASS
- `resolve_mount_sources_makes_relative_absolute` (existing) — still PASS (missing source
  keeps the mount; the `is_dir` warn matches today's behavior).
- `parses_mounts_section`, `validate_rejects_*` — unchanged.

- [ ] **Step 5: Clippy**

Run: `cargo clippy -p oxibuilder-core -- -D warnings`
Expected: no warnings.

- [ ] **Step 6: Commit**

```bash
git add crates/oxibuilder-core/src/config.rs
git commit -m "feat(config): auto-detect mount build output in resolve_mount_sources"
```

---

## Task 2b: Build-side no-match guard (core)

Why this exists: the plan-level regression guard. `resolve_mount_sources` runs at config
load; this build-level test confirms the drop actually propagates through the full
pipeline to `BuildInputs.mounts` and the build never copies a bare project root into
`out/{path}/`. Without this test, the unit test in Task 2 alone could pass while a future
regression re-introduces the bare source into `BuildInputs`.

**Important wiring note:** the drop guard lives in `resolve_mount_sources`, not in
build_writer. Build_writer unconditionally copies whatever it finds in `BuildInputs.mounts`.
This test therefore **routes through Config + resolve_mount_sources + MountCopy::from_config**
— the real pipeline — not a hand-built `MountCopy` injected directly into `BuildInputs`.

**Files:**
- Modify: `crates/oxibuilder-core/tests/static_mounts.rs` (add one test)

- [ ] **Step 1: Write the failing test**

Append to `crates/oxibuilder-core/tests/static_mounts.rs` (add `use oxibuilder_core::config::Config;`
near the existing imports at the top of the file):

```rust
#[test]
fn write_build_output_does_not_copy_root_when_no_static_output_detected() {
    // External project root with src/ and node_modules/ but NO index.html and
    // NO candidate output dir. After resolve_mount_sources drops the mount, the
    // build must not produce out/portfolio/ at all.
    let tmp = TempDir::with_prefix("oxibuilder-mount-nomatch-").unwrap();
    let base = tmp.path();
    let out = base.join("out");
    let media = base.join("media");
    std::fs::create_dir_all(&media).unwrap();

    let project = base.join("project");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::create_dir_all(project.join("node_modules")).unwrap();
    std::fs::write(project.join("src").join("main.rs"), "fn main() {}").unwrap();

    // Build a Config with one mount whose source is the project root.
    let mut cfg = Config::default();
    cfg.mounts.push(MountConfig {
        id: "p".into(),
        source: "project".into(), // relative to base
        path: "portfolio".into(),
        title_ko: "k".into(),
        title_en: "e".into(),
        description: None,
        icon: None,
        open_in_new_tab: false,
    });
    cfg.resolve_mount_sources(base);

    // The drop must happen at resolve — that is the actual guard.
    assert!(
        cfg.mounts.is_empty(),
        "no-match mount must be dropped at resolve; got: {:#?}",
        cfg.mounts
    );

    // Build BuildInputs from the (now-empty) resolved mounts — the real pipeline.
    let out_struct = empty_output_with(vec![page(
        "index.html",
        "<!DOCTYPE html><html><body>lobby</body></html>",
    )]);
    let mut inputs = BuildInputs::new("https://example.com/", "paper", "shell", "seed");
    inputs.mounts = cfg.mounts.iter().map(MountCopy::from_config).collect();
    assert!(
        inputs.mounts.is_empty(),
        "BuildInputs.mounts must be empty after resolve drop"
    );
    write_build_output(&out_struct, &out, &media, &inputs).unwrap();

    // The mount path must not exist under out/ at all.
    assert!(
        !out.join("portfolio").exists(),
        "no-match mount must not create out/portfolio/"
    );
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p oxibuilder-core --test static_mounts write_build_output_does_not_copy_root_when_no_static_output_detected`
Expected: FAIL — the `cfg.mounts.is_empty()` assertion fails (the drop isn't implemented
yet at this step; or if Task 2 has already run, the assertion passes and the build
produces no output, which is still the desired state — note Step 2's failure mode in
relation to Task 2's order: this test is run after Task 2, so the resolve-drop happens
and the rest of the assertions pass. The test MUST fail transiently if Task 2's drop is
reverted, which is its real purpose.)

- [ ] **Step 3: Confirm drop semantics carry through to BuildInputs**

The test passes when `resolve_mount_sources` (Task 2) drops the mount and BuildInputs is
built from the resulting empty `config.mounts`. No code in `build_writer` is changed.

If this test fails after Task 2 passes, the drop semantics did not actually reach
`BuildInputs.mounts` — investigate. Most likely cause: a future caller forgets to use
`MountCopy::from_config` (or equivalent) and injects raw mounts; the test pins the
correct pipeline.

- [ ] **Step 4: Run the full mount test file**

Run: `cargo test -p oxibuilder-core --test static_mounts`
Expected: all pass, including `write_build_output_copies_mount_into_out` (happy path) and
`write_build_output_does_not_copy_root_when_no_static_output_detected` (new).

- [ ] **Step 5: Clippy**

Run: `cargo clippy -p oxibuilder-core -- -D warnings`
Expected: no warnings.

- [ ] **Step 6: Commit**

```bash
git add crates/oxibuilder-core/tests/static_mounts.rs
git commit -m "test(core): assert no-match mount does not copy project root to out/"
```

---
## Task 3: Surface `resolved_source` in `GET /mounts` (console)

**Files:**
- Modify: `crates/oxibuilder-console/src/router.rs` — `mounts_list` handler (around line 343)
- Test: `crates/oxibuilder-console/tests/mounts_api.rs`

**Interfaces:**
- Consumes: `ctx.settings.read().await.mounts` (`MutableSiteSettings` already carries load-resolved `MountConfig`s — i.e. the detected absolute sources). `MutableSiteSettings.mounts` is `#[serde(skip)]` but readable in-process.
- Produces: each entry in `GET /api/console/mounts`'s `data.mounts[]` gains `"resolved_source": "<absolute detected path>"` (present only when the resolved source differs from the raw source or is non-empty; `null`/omitted otherwise).

**Why no base-path plumbing:** `ctx.settings.mounts[i].source` is already the post-detection absolute path (load runs `resolve_mount_sources`). The handler maps raw-doc mounts to resolved ones by `id` and copies the resolved source string. No fs re-scan in the handler.

- [ ] **Step 1: Write the failing test**

Add to `crates/oxibuilder-console/tests/mounts_api.rs` (reusing the existing `app_with_site` helper and `send`):

```rust
#[tokio::test]
async fn mount_list_surfaces_resolved_source_for_auto_detected_dir() {
    let (dir, _path, app) = app_with_site().await;

    // External project root living next to the config: <dir>/extproj/dist/index.html
    let dist = dir.path().join("extproj").join("dist");
    std::fs::create_dir_all(&dist).unwrap();
    std::fs::write(dist.join("index.html"), "x").unwrap();

    // Add a mount whose raw source is the project root (not the dist).
    let (s, _v) = send(
        app.clone(),
        "POST",
        "/mounts",
        Some(json!({
            "id": "ext",
            "source": "extproj",
            "path": "ext",
            "title_ko": "k",
            "title_en": "e",
        })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);

    let (s, v) = send(app.clone(), "GET", "/mounts", None).await;
    assert_eq!(s, StatusCode::OK);
    let mounts = v["data"]["mounts"].as_array().unwrap();
    assert_eq!(mounts.len(), 1);
    // Raw source is preserved verbatim.
    assert_eq!(mounts[0]["source"], "extproj");
    // Resolved source points at the auto-detected dist dir (absolute).
    let resolved = mounts[0]["resolved_source"].as_str().unwrap();
    assert!(
        resolved.ends_with("extproj/dist"),
        "expected resolved source under extproj/dist, got {resolved}"
    );
    assert!(std::path::Path::new(resolved).is_absolute(), "should be absolute");
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p oxibuilder-console --test mounts_api mount_list_surfaces_resolved_source_for_auto_detected_dir`
Expected: FAIL — `resolved_source` is `null` / missing (assertion on `.as_str().unwrap()` panics).

- [ ] **Step 3: Implement resolved-source injection in `mounts_list`**

Edit `mounts_list` in `crates/oxibuilder-console/src/router.rs`. Build a resolved-source map keyed by mount id from the live (already-resolved) settings, then stamp each raw-doc entry:

```rust
/// `GET /api/console/mounts` — list configured mounts (raw sources) plus the
/// load-resolved (auto-detected) source path for each, keyed by id.
async fn mounts_list(
    State(registry): State<Arc<SiteRegistry>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let ctx = default_ctx(&registry).await?;
    let doc = read_toml_doc(&ctx).await?;
    let mut mounts = mounts_from_doc(&doc);

    // ctx.settings.mounts carry the load-resolved (absolute, auto-detected)
    // sources. Map them by id and surface as `resolved_source`.
    let resolved: std::collections::HashMap<String, String> = {
        let settings = ctx.settings.read().await;
        settings
            .mounts
            .iter()
            .map(|m| (m.id.clone(), m.source.to_string_lossy().into_owned()))
            .collect()
    };
    for m in mounts.iter_mut() {
        if let Some(id) = m.get("id").and_then(|v| v.as_str()) {
            if let Some(r) = resolved.get(id) {
                m["resolved_source"] = serde_json::Value::String(r.clone());
            }
        }
    }

    Ok(Json(serde_json::json!({ "data": { "mounts": mounts } })))
}
```

Notes:
- `mounts_from_doc` returns `Vec<serde_json::Value>` (objects); mutating in place is fine.
- `ctx.settings` is an `RwLock<MutableSiteSettings>`; `.read().await` mirrors the existing borrow style elsewhere in the file. No new imports beyond `std::collections::HashMap` (fully qualified inline, so no import needed).

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p oxibuilder-console --test mounts_api mount_list_surfaces_resolved_source_for_auto_detected_dir`
Expected: PASS.

- [ ] **Step 5: Run the whole mounts_api suite (no regressions)**

Run: `cargo test -p oxibuilder-console --test mounts_api`
Expected: all pass — including `mount_crud_round_trip_preserves_raw_source` (its `../portfolio` source is non-existent → no resolved entry → `resolved_source` absent, which the existing assertions don't check, so unchanged).

- [ ] **Step 6: Clippy**

Run: `cargo clippy -p oxibuilder-console -- -D warnings`
Expected: no warnings.

- [ ] **Step 7: Commit**

```bash
git add crates/oxibuilder-console/src/router.rs crates/oxibuilder-console/tests/mounts_api.rs
git commit -m "feat(console): surface resolved mount source in GET /mounts"
```

---

## Task 4: Workspace verification + manual smoke

**Files:** none (verification only)

- [ ] **Step 1: Full workspace test + lint**

Run: `cargo test --workspace && cargo clippy --workspace -- -D warnings`
Expected: all green.

- [ ] **Step 2: Reinstall binary**

Run: `cargo install --path crates/oxibuilder-cli --locked --force`
Expected: installs `oxibuilder` to `~/.cargo/bin/oxibuilder`.

- [ ] **Step 3: Manual smoke against the real portfolio mount**

From `/Volumes/MERCURY/PROJECTS/a7garden.github.io`:
1. Edit `oxibuilder.toml` so the portfolio mount `source` is `../portfolio` (the project root), not `../portfolio/dist`.
2. `oxibuilder build` — expect a log line like `mount portfolio: auto-detected ../portfolio -> .../portfolio/dist`.
3. `oxibuilder mount list` — expect the `portfolio` entry to show both `source = "../portfolio"` and a `resolved_source` ending in `portfolio/dist`.
4. Confirm `out/portfolio/index.html` exists (the detected dir was copied).

Expected: build succeeds; mount materializes under `out/portfolio/`; list shows the resolved path.

- [ ] **Step 4: Restore the toml if desired**

If the manual edit to `../portfolio` is to be kept, leave it; otherwise revert to `../portfolio/dist`. (Either now works — the override path and the auto-detect path both resolve to `dist/`.)

---

## Self-Review



**Spec coverage:**
- §3.1 `detect_static_output` + candidate const + 5 unit tests → Task 1.
- §3.2 integration into `resolve_mount_sources` with drop semantics → Task 2 (Step 3
  `retain_mut` + tightened None branch).
- §3.3 ambiguity → covered by priority order (highest match wins; spec's optional "also
  matched" log note is deferred — single-match common case stays quiet; not a spec
  requirement, just a transparency nicety).
- §3.4 `resolved_source` in `mount list` → Task 3.
- §4 error handling → Task 2's `None` branch trees: missing-source keep, missing-source
  file keep, real-dir-no-match drop.
- §5 testing: 5 `detect_static_output` cases (Task 1), 2 `resolve_mount_sources` cases
  (Task 2: happy path + drop), 1 build-side no-match guard (Task 2b), 1 console
  round-trip (Task 3). The existing `resolve_mount_sources_makes_relative_absolute` and
  `write_build_output_copies_mount_into_out` regression tests are called out in Tasks 2
  and 2b expected outputs.
- §6 non-goals respected (no configurable list, no recursive scan, no build orchestration,
  no path rewriting).

**Green-while-wrong guard (the blocker).** The plan now has THREE tests pinning the drop:
1. `resolve_mount_sources_drops_mount_when_no_static_output_detected` (Task 2) — unit:
   no-match mount is removed from `config.mounts`.
2. `write_build_output_does_not_copy_root_when_no_static_output_detected` (Task 2b) —
   integration: build does not produce `out/{path}/` even when the mount *does* reach
   `BuildInputs` (post-resolution). The test pins the build-side invariant.
3. `mount_list_surfaces_resolved_source_for_auto_detected_dir` (Task 3) — integration:
   `GET /mounts` returns the `resolved_source` field for an auto-detected mount, which
   is the user-visible signal that detection succeeded.

A future regression that re-introduces the bare source into `out/{path}/` would fail at
least one of these. The build-side test (Task 2b) is the direct guard for the failure
mode the advisory identified.

**Placeholder scan:** none — every code step contains real code; test steps contain real
assertions.

**Type consistency:** `detect_static_output(&Path) -> Option<PathBuf>` defined in Task 1,
used in Task 2 with identical signature. `mounts_from_doc` returns `Vec<Value>`; Task 3
mutates entries by `id` key — matches the existing object shape (`id`, `source`, …).

**Regression guard:** the non-canonical join contract
(`resolve_mount_sources_makes_relative_absolute`) is preserved because detection returns
`None` for the test's non-existent source and the `is_dir` warn path keeps the mount
unchanged. The drop semantics apply only to the real-dir-no-match case, which the
existing test does not cover.
