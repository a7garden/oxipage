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
4. **Root-as-result is a fallback, not a short-circuit.** Scan candidates first. Only if
   no candidate matches, fall back to `source` itself as the result dir. This is what
   distinguishes a Vite project root (root `index.html` + `dist/`) — where the candidate
   scan correctly returns `dist/` — from a hand-built static folder (only `index.html`,
   no candidates) — where the fallback returns the source itself. Inverting the order
   (root-first) silently copies `node_modules/`, `src/`, etc. into `out/{path}/` for any
   Vite/vanilla project, recreating the green-while-wrong bug the drop semantics prevent.

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

/// Locate a mount's static build output under `source`.
///
/// Candidates are scanned first (`MOUNT_OUTPUT_CANDIDATES` in priority order). The
/// first `source/<candidate>` that is a directory containing `index.html` wins. If no
/// candidate matches, `source` itself is returned as the result dir — this is the
/// exact-path override (`source = "../portfolio/dist"` is honored verbatim) and
/// also covers hand-built static folders where no `dist/` etc. exists. Returns
/// `None` only when neither scan finds a result.
fn detect_static_output(source: &Path) -> Option<PathBuf>;
```

Resolution order, in one pass over a single mount's already-absolute `source`:

1. Iterate `MOUNT_OUTPUT_CANDIDATES`: the first `source.join(candidate)` that is a
   directory containing `index.html` wins. **This is the dominant case for a Vite/Astro
   project root** — the root has an `index.html` entry template, but the candidate-scanned
   `dist/` is the actual build output.
2. If no candidate matches, fall back to `source` itself on the basis that it directly
   contains `index.html`. This handles the exact-path override (`../portfolio/dist`)
   and hand-built static folders.
3. Else `None` — drop path (resolve_mount_sources §3.2).

### 3.2 Integration into `resolve_mount_sources`

`Config::resolve_mount_sources(base)` (config.rs) currently just makes each `source`
absolute and warns if it is not a dir. It gains a detection step after the absolutization
**and drops the mount from `self.mounts` when no output is detected** — otherwise the
downstream `copy_dir_recursive` (build_writer.rs) would recursively copy the entire project
root (including `node_modules`, `.git`, `src`, …) into `out/{path}/`.

```rust
pub fn resolve_mount_sources(&mut self, base: &Path) {
    self.mounts.retain_mut(|m| {
        if !m.source.is_absolute() {
            m.source = base.join(&m.source);
        }
        // m.source is now absolute. Auto-detect the build output under it.
        match detect_static_output(&m.source) {
            Some(resolved) if resolved != m.source => {
                tracing::info!(
                    "mount {}: auto-detected {} -> {}",
                    m.id, m.source.display(), resolved.display()
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
                    // will hard-error on the copy if the source is still missing
                    // then, which is the same as today's failure mode.
                    tracing::warn!("mount {} source not found: {}", m.id, m.source.display());
                    true
                } else {
                    // Source dir exists but no static output detected — this is
                    // the dangerous case: leaving the bare project root in
                    // `config.mounts` would let `copy_dir_recursive` copy
                    // `node_modules`, `.git`, `src`, … into `out/{path}/`. Drop.
                    tracing::warn!(
                        "mount {}: no static output detected under {} \
                         (looked for index.html and: {}) — dropping mount",
                        m.id, m.source.display(), MOUNT_OUTPUT_CANDIDATES.join(", ")
                    );
                    false
                }
            }
        }
    });
}
```

Drop semantics: the failing mount is removed from `self.mounts` in memory only. The
underlying `oxibuilder.toml` is **untouched** (raw-doc patching already preserves the
verbatim entry; the next load re-detects from the raw `source`). All three downstream build
call sites (CLI `build.rs`, console `build_run.rs`, core `http.rs`) read `config.mounts`
after `resolve_mount_sources`, so they inherit the drop automatically — no call-site changes.

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

- **source missing entirely** → warn (non-fatal at load). The mount is kept in
  `self.mounts`. The build will hard-error on the copy, exactly as today — this preserves
  the existing `resolve_mount_sources_makes_relative_absolute` test and the existing
  build-side failure mode for misconfigured mounts.
- **source is a file** → warn (`is_dir` false). Kept in `self.mounts`; build will hard-error.
- **source is a dir but no candidate + no root index.html** → warn naming what was looked
  for (§3.2) **and the mount is dropped from `self.mounts`**. This is the critical case:
  leaving the bare project root in `config.mounts` would let `copy_dir_recursive` copy
  `node_modules`, `.git`, `src`, … into `out/{path}/`. The drop is the guard.
- Detection never panics and never changes `path` / `id` / validation behavior.

**Why drop only on real-dir-no-match, not on missing source.** The missing-source path is
already caught with a hard error at build time (build_writer.rs reports the missing source
with mount id + path). Silently dropping it from the in-memory snapshot adds no value and
would silently break the existing `resolve_mount_sources_makes_relative_absolute` test
without changing observable failure behavior. The drop case is precisely the one that
would *not* error downstream — the source dir exists, but its contents are not what the
user meant for `out/{path}/`, so the in-memory snapshot must protect the build.

**Visibility for dropped mounts.** When a mount is dropped at load, the build does not
copy anything under `out/{path}/`. The `mount list` endpoint cannot distinguish "kept" from
"dropped" without re-running detection in the handler (which already happens for
`resolved_source`); see §3.4 — the `resolved_source` is `null`/absent for dropped mounts,
which is the existing list semantics. A future "skipped" badge is out of scope.

## 5. Testing

`config.rs` `#[cfg(test)]` for the unit core:

- Root override (only candidate): `source` directly containing `index.html` with **no
  candidate subdir** → returns `source`.
- **Candidate wins over root `index.html` (green-while-wrong guard):** source has BOTH a
  root `index.html` AND a `dist/index.html` (the Vite/vanilla project root case) → returns
  `dist`. Without this test, the inverted order silently copies `node_modules/` into
  `out/{path}/` for Vite users.
- Priority: a temp source with both `dist/index.html` and `build/index.html` (and no
  root `index.html`) → returns `dist`.
- `resolve_mount_sources` happy path: relative source + a temp dir tree → `m.source` becomes
  the detected absolute dir.
- `resolve_mount_sources` **drop semantics**: a source that is a real directory but lacks
  `index.html` and any candidate output subdir → mount is removed from `self.mounts`. The
  existing `resolve_mount_sources_makes_relative_absolute` non-canonical-path test still
  passes (its source is non-existent; the `is_dir` warn path *keeps* the mount, matching the
  prior behavior).

`build_writer.rs` (test file `crates/oxibuilder-core/tests/static_mounts.rs`) for the
build-side guard against the green-while-wrong risk:

- **No-match mount produces no copy under `out/{path}/`.** A mount whose source is a real
  directory but lacks `index.html` and any candidate output subdir must not create
  `out/{path}/` — in particular, must not copy the source root itself into `out/`.
- The existing `write_build_output_copies_mount_into_out` (which asserts a happy-path
  mount copy) still passes — `detect_static_output` returns the source itself when it
  contains `index.html`, so the copy behaves exactly as before.

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
