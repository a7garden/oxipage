# GitHub Pages Console Deploy — Design Spec

> **Date:** 2026-07-31
> **Subproject:** 4 of 5
> **Predecessors:** Runtime/routing foundation; built preview static-base contract
> **Supported targets:** GitHub Pages root and project repositories

## 1. Goal

Turn GitHub Pages deployment into a configured, preflighted, observable site operation. The console must deploy the selected site's build from the selected site's repository, support both root and project Pages URLs, preserve `gh` CLI authentication, and retain status/history across reloads.

## 2. Current state

A real deploy pipeline already exists:

```text
DeployPage
→ POST /api/console/s/{slug}/deploy
→ DeployGuard
→ GET /deploy/{run_id}/stream
→ oxibuilder_deploy::deploy_github_pages
```

The named `deploy/site_deploy.rs` file is a separate dead top-level stub; the SPA does not use it.

Load-bearing defects:

- deploy core assumes process CWD is the site Git repository,
- `/tmp/oxibuilder-deploy-{pid}` collides across concurrent sites,
- branch is hard-coded,
- shell commands interpolate strings through `bash -c`,
- no owner/repository/base config exists,
- static output is not project-base compatible,
- no deploy history is persisted,
- build and deploy use separate guards and may race on the same output,
- CLI parses `--site` but does not use it,
- the Dashboard/DeployPage imply deploy state not backed by a deploy record.

## 3. Scope

### In scope

- Per-site GitHub Pages owner/repository/branch config.
- Root and project Pages URL/base derivation.
- Settings configuration and DeployPage preflight.
- Correct repository-scoped deploy core shared by CLI and console.
- A single per-site build/deploy operation guard.
- SSE reattachment and deploy history.
- Clean removal of the top-level deploy stub and legacy top-level build route.

### Out of scope

- Cloudflare, Netlify, or other targets.
- Custom domains/CNAME.
- Scheduled deploys, rollback, or release approvals.
- Storing GitHub credentials.
- Creating GitHub repositories automatically.

## 4. Configuration

Add to `oxibuilder.toml`:

```toml
[deploy.github_pages]
owner = "a7garden"
repo = "oxibuilder-site"
branch = "gh-pages"
```

Rust model:

```rust
pub struct DeployConfig {
    pub github_pages: Option<GitHubPagesTarget>,
}

pub struct GitHubPagesTarget {
    pub owner: String,
    pub repo: String,
    pub branch: String,
}
```

Validation:

- owner/repo: GitHub-compatible alphanumeric, `-`, `_`, `.`, non-empty; reject separators and shell metacharacters,
- branch: conservative `[A-Za-z0-9._/-]+`, reject `..`, leading/trailing slash, control characters, and shell interpretation,
- default branch: `gh-pages`.

The existing site-scoped config API is extended rather than adding a second settings endpoint:

```text
GET /api/console/s/{slug}/config
PUT /api/console/s/{slug}/config
```

`ConfigResponse` gains `deploy.github_pages`; the allowlisted `ConfigUpdate` gains `deploy` but never accepts `server.host`, `server.port`, or `server.data_dir`. PUT follows the foundation contract: lock, reread current TOML, patch the mutable deploy table while preserving server/unknown keys, validate, atomically rename, then replace `SiteContext.settings`. Preflight and the next build observe saved deploy settings immediately; listener, DB pool, and resolved paths remain startup-immutable.

Derived values:

```text
repo == "<owner>.github.io"
  pages_url = https://<owner>.github.io/
  base_path = /
otherwise
  pages_url = https://<owner>.github.io/<repo>/
  base_path = /<repo>/
```

`site.base_url` must match the derived Pages URL for this target. Settings provides an explicit “Use this Pages URL as Site Base URL” action; it is never changed silently.

## 5. Settings UX

`SettingsPage > Deployment > GitHub Pages` contains:

- Owner,
- Repository,
- Publish branch,
- derived Pages URL,
- derived base path,
- `gh` installed/authenticated status,
- repository/origin status,
- Save configuration,
- Open Deploy page.

`integrations.github_username` remains separate because it is used for Activity/Profile data. It is not reused as deploy owner implicitly.

No token/password field exists.

Owner, repository, and branch are saved together through the site config mutation. Partial invalid targets are displayed as unsaved field errors and never persisted.

## 6. Preflight

```text
GET /api/console/s/{slug}/deploy/preflight
```

Response:

```json
{
  "data": {
    "configured": true,
    "gh_installed": true,
    "authenticated": true,
    "git_repository": true,
    "origin_matches": true,
    "build_compatible": true,
    "pages_url": "https://a7garden.github.io/oxibuilder-site/",
    "base_path": "/oxibuilder-site/",
    "problems": []
  }
}
```

Checks:

1. valid target config,
2. `gh --version`,
3. `gh auth status`,
4. `project_dir/.git` is a repository,
5. origin points to configured owner/repo,
6. `out_dir/.oxibuilder-build.json` exists,
7. manifest deployment base equals target base,
8. manifest theme ID equals current site theme,
9. `out/index.html` and referenced assets exist.

The Deploy button is disabled until all load-bearing checks pass. Problems include a code, user-facing message, and remediation action where one is safe.

## 7. Shared deploy-core contract

```rust
pub fn deploy_github_pages(
    repo_dir: &Path,
    out_dir: &Path,
    target: &GitHubPagesTarget,
    manifest: &BuildManifest,
    tx: &mpsc::Sender<DeployEvent>,
) -> Result<DeployOutcome, DeployError>;
```

Rules:

- Every git/gh command uses `Command::current_dir(repo_dir)` or explicit `--git-dir/--work-tree` paths.
- The origin is verified against configured owner/repo.
- No `bash -c`, `cp`, or `rm` command string is used.
- Files are copied through Rust filesystem APIs.
- Worktree path is generated uniquely with UUID in a safe temp directory.
- A cleanup guard removes/prunes the worktree on success, failure, and panic unwinding.
- Branch is the validated configured branch.
- Empty/no-change commit returns `DeployOutcome::Unchanged`, not failure.
- A mismatched build manifest returns a typed precondition error before modifying git state.
- Successful URL is the derived Pages URL, not the raw git remote URL.

Events:

```text
PreflightStarted
GhReady
AuthReady
RepositoryReady
WorktreeReady
FilesCopied { count }
CommitCreated { commit }
Pushing { branch }
Deployed { url, commit }
Unchanged { url, commit }
Failed { code, error }
```

## 8. Operation guard and run lifecycle

Build and deploy both touch/read `out_dir`; they share one site operation guard:

```rust
enum SiteOperationKind { Build, Deploy }
```

Only one build or deploy may run per site. A conflicting request returns:

```http
409
{
  "error": "site_operation_in_progress",
  "kind": "build",
  "run_id": "..."
}
```

Different sites may operate concurrently. Unique worktree paths prevent collisions.

Run state retains the terminal outcome long enough for reconnect, while durable history is written to the DB. A client refreshing mid-run queries current operation and reattaches to SSE.

## 9. Deploy history

Create per-site `deploy_log`:

```text
id INTEGER PRIMARY KEY
run_id TEXT UNIQUE
build_id TEXT
target TEXT
owner TEXT
repo TEXT
branch TEXT
base_path TEXT
status TEXT
url TEXT
commit_sha TEXT
error_code TEXT
error TEXT
started_at TEXT
finished_at TEXT
```

Statuses:

```text
running | deployed | unchanged | failed
```

APIs:

```text
GET /api/console/s/{slug}/deploys?limit=50
GET /api/console/s/{slug}/operations/current
```

History records all terminal paths, including failure and unchanged. Secrets and command stderr containing credentials are never persisted; errors are normalized.

`stats` gains `last_deploy`. Dashboard and DeployPage remove hard-coded deployment text and consume it.

## 10. DeployPage UX

Sections:

1. Header actions: Build, Preview Site, Deploy.
2. Preflight status card with remediation links.
3. Current live operation log; reattached after reload.
4. Last deployment: status, URL, commit, duration, build ID.
5. Build history.
6. Deploy history.

Behavior:

- 424/no output directs the user to Build.
- Stale theme/base manifest directs the user to Rebuild.
- 409 attaches to current operation regardless of whether it is build or deploy.
- Success/unchanged exposes “Open site”.
- Failure shows normalized error and Retry after preflight passes.

## 11. CLI convergence

`oxibuilder deploy --site <slug>`:

- reads `SitesFile`,
- resolves the selected `project_dir` and config through the same path resolver,
- reads the same build manifest and target,
- calls the same deploy core,
- prints the same events.

Without `--site`, use the registered default site when a SitesFile exists; legacy standalone config resolution is allowed only when there is no registry. The parsed `--site` is no longer ignored.

## 12. File map

```text
crates/oxibuilder-core/src/config.rs              # deploy config
crates/oxibuilder-deploy/src/lib.rs                # repo-scoped safe deploy
crates/oxibuilder-console/src/
├── operations.rs                               # shared guard/current run
├── deploy/deploy_run.rs                        # history/outcome/SSE
├── per_site.rs                                 # preflight/deploys/current routes
└── router.rs                                   # no legacy stub
crates/oxibuilder-cli/src/commands/deploy.rs        # honor --site/shared core
web/src/admin/
├── shared/api.ts                               # config/preflight/history/current
├── settings/SettingsPage.tsx                   # Deployment section
├── deploy/DeployPage.tsx                       # preflight/live/history
└── dashboard/DashboardPage.tsx                 # real last deploy
```

## 13. Verification

- Root target derives `/`; project target derives `/<repo>/`.
- Invalid owner/repo/branch values are rejected before commands execute.
- Console launched outside a site repository deploys the selected site's repo only.
- Origin mismatch and unauthenticated `gh` fail preflight without git modifications.
- Manifest base/theme mismatch requires rebuild.
- Build during deploy and deploy during build return 409; another site remains operable.
- Two sites deploying concurrently use distinct worktrees.
- Refresh during deploy reattaches to the run.
- Success, unchanged, and failure each persist correct history.
- Root and project temporary remotes receive output whose assets/data/media resolve at their Pages URL.
- CLI `--site` selects the requested registered site.
