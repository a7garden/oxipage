# Static Mounts — Source Auto-Detection

**Date:** 2026-08-07
**Status:** Draft (pending user spec-review)
**Depends on:**
- `2026-08-06-static-mounts-design.md` (merged) — mounts exist as `[[mounts]]` in
  `oxibuilder.toml`, copied into `out/{path}/` at build time.
- `2026-08-06-mount-cli-design.md` (merged) — `oxibuilder mount add/list/rm` manage mounts
  over HTTP; raw-doc patching keeps the stored `source` verbatim.

## 1. Problem

Today a mount's `source` must point at the *exact* build-output directory — e.g.
`source = "../portfolio/dist"`. The user has to know where each external tool drops its
artifacts (Astro/Vite → `dist`, Next export → `out`, Nuxt → `.output/public`, Jekyll →
`_site`). Pointing at the project root (`../portfolio`) fails: `resolve_mount_sources`
treats it as the result dir, finds no `index.html` at the root, and at build time the copy
produces an empty/useless mount.

The user wants to give a **folder** (typically the external project root) and have oxibuilder
locate the static build output under it automatically, so they do not have to remember each
tool's output path.

## 2. Decisions (from brainstorming)

1. **When detection runs — every build (not frozen at `add` time).** The toml keeps the raw
   `source` the user wrote (e.g. `../portfolio`); detection runs on each config load / build.
   External projects can be rebuilt or change their output location and oxibuilder tracks
   automatically. Rejected: detect-once-and-freeze into the toml — explicit but breaks on
   external rebuild/relocation, and the toml is already the verbatim source of truth by design.
2. **Detection algorithm — name matching (A).** Look for well-known build-output directory
   names by priority, validated by `index.html`. Rejected: recursive content scan — slower,
   unpredictable which dir wins. Standard tools have conventional output names, so name
   matching is precise and fast.
3. **`public` / `site` excluded from candidate names.** These are commonly build inputs or
   multi-purpose; matching them risks grafting the wrong directory.
4. **Exact override still works.** A `source` that already points at a real result dir
   (contains `index.html`) is used as-is; detection is skipped.

## 3. Design

### 3.1 Detection function — `crates/oxibuilder-core/src/config.rs`

A pure helper (filesystem-touching but allocation-light, trivially unit-testable):

```rust
/// Candidate build-output directory names in priority order.
/// NOTE: `public`/`site` are deliberately omitted — they are commonly build
/// inputs or multi-purpose, so matching them risks grafting the wrong dir.
const MOUNT_OUTPUT_CANDIDATES: &[&str] = &[
    "dist",          // Vite / Astro / most bundlers
    "build",         // CRA / others
    "out",           // Next.js static export
    ".output/public",// Nuxt / Nitro (2-deep)
    "_site",         // Jekyll / eleventy
    "www",           // assorted
];

/// If `source` itself is a static-site root (contains index.html), return it
/// unchanged (explicit override). Otherwise search `source/<candidate>` for the
/// first candidate that is a directory containing `index.html`. Returns the
/// resolved dir, or None if nothing matched.
fn detect_static_output(source: &Path) -> Option<PathBuf>;
```

Resolution order, in one pass over a single mount's already-absolute `source`:

1. `source/index.html` exists → `source` is the result dir. Return `source`. (This is also
   the exact-path override: a `source` like `../portfolio/dist` that really holds the output
   is honored verbatim.)
2. Else iterate `MOUNT_OUTPUT_CANDIDATES`: the first `source.join(candidate)` that is a
   directory and contains `index.html` wins.
3. Else `None`.

### 3.2 Integration into `resolve_mount_sources`

`Config::resolve_mount_sources(base)` (config.rs) currently just makes each `source`
absolute and warns if it is not a dir. It gains a detection step after the absolutization:

```rust
pub fn resolve_mount_sources(&mut self, base: &Path) {
    for m in &mut self.mounts {
        if !m.source.is_absolute() {
            m.source = base.join(&m.source);
        }
        // source is now absolute. Auto-detect the build output under it.
        match detect_static_output(&m.source) {
            Some(resolved) if resolved != m.source => {
                tracing::info!(
                    "mount {}: auto-detected {} -> {}",
                    m.id, m.source.display(), resolved.display()
                );
                m.source = resolved;
            }
            Some(_) => {
                // source itself is the result dir (override); nothing to log.
            }
            None => {
                if !m.source.is_dir() {
                    tracing::warn!("mount {} source not found: {}", m.id, m.source.display());
                } else {
                    tracing::warn!(
                        "mount {}: no static output detected under {} \
                         (looked for index.html and: {})",
                        m.id, m.source.display(), MOUNT_OUTPUT_CANDIDATES.join(", ")
                    );
                }
            }
        }
    }
}
```

Because every build path (CLI `build`, console `build_run`, on-demand `http.rs`) reads
`config.mounts` through `resolve_mount_sources`, detection applies uniformly to all of them.

**Toml is not rewritten.** Raw-doc patching (`mounts_add`) stores the verbatim `source`; the
resolved path lives only in memory for the build. A subsequent load re-detects from the raw
source, so an external rebuild / output-path change is picked up automatically.

### 3.3 Ambiguity

If two or more candidates contain `index.html`, the priority order picks one
deterministically. To keep it transparent, when a non-top-priority candidate matches the log
also names the skipped candidates:

```text
mount portfolio: auto-detected ../portfolio -> ../portfolio/build
  (also matched: dist)  // only logged when a lower-priority match was chosen over a higher one
```

(Implementation detail: collect all matches, emit the note only if the winner is not the
highest-priority match present. Keeps the common single-match case quiet.)

### 3.4 Visibility — `mount list`

`GET /api/console/mounts` and `oxibuilder mount list` currently return only the raw
`source` (exactly as written in the toml). That is correct for the source-of-truth view, but
the user also wants to see what detection chose. Add a `resolved_source` field to the list
response:

```json
{
  "data": {
    "mounts": [
      { "id": "portfolio", "source": "../portfolio", "resolved_source": "/abs/portfolio/dist", ... }
    ]
  }
}
```

`resolved_source` is computed by running `detect_static_output` on the already-resolved
absolute source for display only (it does not mutate the toml). Omitted (or null) when
detection found nothing. `mount_table_to_json` (router.rs) gains no field from the toml; the
handler resolves and injects it. The CLI prints it alongside `source` in human mode and
includes it under `--json`.

## 4. Error handling

- **source missing entirely** → existing behavior: warn (non-fatal at load), hard error at
  build (cannot copy).
- **source is a file** → warn (`is_dir` false).
- **source is a dir but no candidate + no root index.html** → warn naming what was looked
  for (§3.2); build then fails on the copy if the dir has no usable content, same as today.
- Detection never panics and never changes `path` / `id` / validation behavior.

## 5. Testing

Pure function `detect_static_output` is the unit-testable core. `config.rs` `#[cfg(test)]`:

- Root override: `source` directly containing `index.html` → returns `source`.
- Priority: a temp source with both `dist/index.html` and `build/index.html` → returns
  `dist`.
- 2-deep candidate: source with `.output/public/index.html` (and nothing else) → matches.
- index.html gating: `source/dist/` exists but lacks `index.html` → skipped; next candidate
  or None.
- No match: source with only `node_modules`, `src`, etc. → None.
- `resolve_mount_sources`: relative source + a temp dir tree → `m.source` becomes the
  detected absolute dir; a non-existent source keeps the old warn path.

No new build/manifest tests — the copy and manifest steps are unchanged downstream of a
resolved `source`.

## 6. Non-goals

- Configurable candidate list (e.g. a `[mounts] output_dirs = [...]` setting). YAGNI; the
  fixed conventional set covers the standard tools. Add a setting only if a real tool is
  missed.
- Detecting inside nested project subdirs beyond the listed candidates (no recursive scan).
- Running the external build tool itself (still copy-only — orchestration is out of scope,
  unchanged from the original static-mounts spec).
- Rewriting absolute asset paths inside the mounted HTML (unchanged: relative paths required).

## 7. Open questions for spec review

- Is the candidate list / priority right? Proposed: `dist > build > out > .output/public >
  _site > www`.
- `www` vs dropping it — `www` is less universal than the others; keep as low priority or
  remove?
