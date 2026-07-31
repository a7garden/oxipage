# GitHub Pages Console Deploy Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make GitHub Pages deployment a configured, preflighted, repository-scoped operation with reconnectable progress and durable history, shared by the console and CLI.

**Architecture:** `oxipage-core` owns target configuration, validation, and URL/base derivation; `oxipage-deploy` owns a blocking, shell-free Git worktree pipeline. The console adds one per-site build/deploy guard, typed preflight, SSE retention, and SQLite history; React and the CLI consume those shared contracts.

**Tech Stack:** Rust 2024 (axum 0.8, tokio, sqlx SQLite, serde, uuid), Git/GitHub CLI subprocesses, React 19, TypeScript, TanStack Query, Vite 7

## Global Constraints

- Support GitHub Pages root (`<owner>.github.io`) and project repositories only.
- Read mutable values only through `ctx.settings.read().await`; read immutable server values only through `ctx.startup_server`. `SiteContext.config` no longer exists after the foundation cutover.
- Resolve paths only through `SiteContext.project_dir`, `out_dir`, and `media_dir`; deployment never depends on process CWD.
- `server.host`, `server.port`, and `server.data_dir` remain excluded from `ConfigUpdate`.
- Owner/repository allow non-empty ASCII alphanumeric, `-`, `_`, `.` only. Branch allows `[A-Za-z0-9._/-]+`, rejects `..` and leading/trailing `/`, and defaults to `gh-pages`.
- Never silently change `site.base_url`; Settings provides an explicit action.
- No credentials or raw command stderr are persisted.
- No `bash -c`, external `cp`, external `rm`, or shell interpolation in deploy core.
- One build or deploy may run per site; different sites remain concurrent.
- `BuildManifest` from `oxipage_core::build_manifest` is the sole build metadata contract.
- Rust integration tests use `cargo test -p <crate> --test <name>`. Frontend verification uses `cd web && npx tsc --noEmit` plus manual smoke; add no frontend test runner.
- Clean cutover: remove separate `BuildGuard`/`DeployGuard` APIs and leave no aliases.

---

## File Structure

```text
crates/oxipage-core/src/
├── config.rs                         # deserialize DeployConfig
└── site_paths.rs                     # validate/derive foundation target types
crates/oxipage-core/migrations/core/
└── 0007_deploy_log.sql               # durable deployment history
crates/oxipage-core/tests/deploy_config.rs
crates/oxipage-deploy/
├── Cargo.toml                         # core + uuid dependencies
├── src/lib.rs                        # safe repository-scoped pipeline
└── tests/github_pages.rs
crates/oxipage-console/src/
├── operations.rs                     # shared operation slot and retained terminal state
├── lib.rs                            # construct shared guard
├── loader.rs                         # pass guard to SiteContext
├── sites_runtime.rs                  # operation_guard field
├── build/build_run.rs                # build via shared guard
├── deploy/deploy_run.rs              # deploy + deploy_log recorder
└── per_site.rs                       # config/preflight/history/current routes
crates/oxipage-console/tests/
├── config_deploy.rs
├── operations.rs
└── deploy_api.rs
crates/oxipage-cli/src/commands/deploy.rs
crates/oxipage-cli/tests/deploy_site.rs
web/src/admin/
├── shared/api.ts
├── deploy/DeployPage.tsx
├── settings/SettingsPage.tsx
└── dashboard/DashboardPage.tsx
```

---

### Task 1: Core target configuration, validation, and derivation

**Files:**
- Modify: `crates/oxipage-core/src/config.rs`
- Modify: `crates/oxipage-core/src/site_paths.rs`
- Create: `crates/oxipage-core/tests/deploy_config.rs`

**Interfaces:**
- Consumes: foundation `DeployConfig { github_pages: Option<GitHubPagesTarget> }`
- Produces: `Config.deploy`; `GitHubPagesTarget::{validate,pages_url,base_path}`

- [ ] **Step 1: Write the failing test**

```rust
// crates/oxipage-core/tests/deploy_config.rs
use oxipage_core::{config::Config, site_paths::GitHubPagesTarget};
fn target(owner: &str, repo: &str, branch: &str) -> GitHubPagesTarget {
    GitHubPagesTarget { owner: owner.into(), repo: repo.into(), branch: branch.into() }
}
#[test]
fn parses_and_derives_pages_targets() {
    let cfg = Config::from_toml_str(r#"
[site]
name="Site"
base_url="https://project-oxi.github.io/oxipage/"
[deploy.github_pages]
owner="a7garden"
repo="notes"
"#).unwrap();
    let pages = cfg.deploy.github_pages.unwrap();
    assert_eq!(pages.branch, "gh-pages");
    assert_eq!(pages.pages_url(), "https://project-oxi.github.io/oxipage/");
    assert_eq!(pages.base_path(), "/notes/");
    let root = target("a7garden", "a7garden.github.io", "pages/v1");
    assert_eq!((root.pages_url(), root.base_path()), ("https://a7garden.github.io/".into(), "/".into()));
}
#[test]
fn rejects_unsafe_values() {
    for value in ["", "owner/repo", "$(id)", "white space"] {
        assert!(target(value, "repo", "gh-pages").validate().is_err());
        assert!(target("owner", value, "gh-pages").validate().is_err());
    }
    for branch in ["", "../main", "/pages", "pages/", "pages shell", "pages;rm"] {
        assert!(target("owner", "repo", branch).validate().is_err(), "{branch}");
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p oxipage-core --test deploy_config`

Expected: FAIL because `Config.deploy` and target methods are absent.

- [ ] **Step 3: Implement the model and rules**

```rust
// config.rs
use crate::site_paths::DeployConfig;
// Add to Config:
#[serde(default)]
pub deploy: DeployConfig,
// Add to Config::default():
deploy: DeployConfig::default(),
```

```rust
// site_paths.rs — retain the foundation structs, adding these annotations/methods
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct DeployConfig {
    #[serde(default)] pub github_pages: Option<GitHubPagesTarget>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GitHubPagesTarget {
    pub owner: String,
    pub repo: String,
    #[serde(default = "default_pages_branch")] pub branch: String,
}
fn default_pages_branch() -> String { "gh-pages".into() }
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TargetValidationError {
    #[error("invalid GitHub owner")] Owner,
    #[error("invalid GitHub repository")] Repo,
    #[error("invalid Git branch")] Branch,
}
impl GitHubPagesTarget {
    pub fn validate(&self) -> Result<(), TargetValidationError> {
        let component = |v: &str| !v.is_empty() && v.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-'|b'_'|b'.'));
        if !component(&self.owner) { return Err(TargetValidationError::Owner); }
        if !component(&self.repo) { return Err(TargetValidationError::Repo); }
        let branch = !self.branch.is_empty() && !self.branch.contains("..")
            && !self.branch.starts_with('/') && !self.branch.ends_with('/')
            && self.branch.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.'|b'_'|b'/'|b'-'));
        if !branch { return Err(TargetValidationError::Branch); }
        Ok(())
    }
    pub fn pages_url(&self) -> String {
        if self.repo.eq_ignore_ascii_case(&format!("{}.github.io", self.owner)) {
            format!("https://{}.github.io/", self.owner)
        } else { format!("https://{}.github.io/{}/", self.owner, self.repo) }
    }
    pub fn base_path(&self) -> String {
        if self.repo.eq_ignore_ascii_case(&format!("{}.github.io", self.owner)) { "/".into() }
        else { format!("/{}/", self.repo) }
    }
}
// MutableSiteSettings::from_config:
deploy: cfg.deploy.clone(),
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p oxipage-core --test deploy_config`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/oxipage-core/src/config.rs crates/oxipage-core/src/site_paths.rs crates/oxipage-core/tests/deploy_config.rs
git commit -m "feat(core): add validated GitHub Pages target config"
```

---

### Task 2: Atomic allowlisted deployment config API

**Files:**
- Modify: `crates/oxipage-console/src/per_site.rs`
- Create: `crates/oxipage-console/tests/config_deploy.rs`

**Interfaces:**
- Consumes: `ctx.settings`, `ctx.startup_server`, `ctx.config_write_lock`, target validation
- Produces: `ConfigUpdate.deploy`; `ConfigResponse.data.deploy.github_pages`

- [ ] **Step 1: Write the failing preservation/validation test**

```rust
// crates/oxipage-console/tests/config_deploy.rs; reuse the concrete router fixture pattern from site_routes.rs
#[tokio::test]
async fn deploy_patch_preserves_server_and_unknown_keys() {
    let fixture = site_router_with_toml(r#"
[site]
name="Site"
base_url="https://old.invalid/"
[server]
port=9123
data_dir="private-data"
[custom]
keep="yes"
"#).await;
    let response = put_json(fixture.app, "/s/blog/config", serde_json::json!({
        "deploy":{"github_pages":{"owner":"a7garden","repo":"notes","branch":"gh-pages"}}
    })).await;
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let saved: toml::Value = toml::from_str(&std::fs::read_to_string(fixture.project_dir.join("oxipage.toml")).unwrap()).unwrap();
    assert_eq!(saved["server"]["port"].as_integer(), Some(9123));
    assert_eq!(saved["custom"]["keep"].as_str(), Some("yes"));
    assert_eq!(saved["deploy"]["github_pages"]["repo"].as_str(), Some("notes"));
}
#[tokio::test]
async fn invalid_target_is_not_persisted() {
    let fixture = site_router().await;
    let path = fixture.project_dir.join("oxipage.toml");
    let before = std::fs::read_to_string(&path).unwrap();
    let response = put_json(fixture.app, "/s/blog/config", serde_json::json!({
        "deploy":{"github_pages":{"owner":"bad/name","repo":"notes","branch":"gh-pages"}}
    })).await;
    assert_eq!(response.status(), axum::http::StatusCode::BAD_REQUEST);
    assert_eq!(std::fs::read_to_string(path).unwrap(), before);
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p oxipage-console --test config_deploy`

Expected: FAIL because deploy is not allowlisted.

- [ ] **Step 3: Add typed patching and response serialization**

```rust
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ConfigUpdate {
    pub site: Option<SiteUpdate>,
    pub lobby: Option<LobbyUpdate>,
    pub integrations: Option<IntegrationsUpdate>,
    pub deploy: Option<DeployUpdate>,
}
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct DeployUpdate { pub github_pages: Option<GitHubPagesTarget> }

fn apply_deploy_patch(doc: &mut toml::Value, patch: Option<DeployUpdate>) -> Result<(), (StatusCode, String)> {
    let Some(patch) = patch else { return Ok(()); };
    if let Some(target) = &patch.github_pages {
        target.validate().map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    }
    let root = doc.as_table_mut().ok_or((StatusCode::INTERNAL_SERVER_ERROR, "root TOML is not a table".into()))?;
    let deploy = root.entry("deploy").or_insert_with(|| toml::Value::Table(toml::Table::new()))
        .as_table_mut().ok_or((StatusCode::BAD_REQUEST, "deploy must be a table".into()))?;
    match patch.github_pages {
        Some(t) => deploy.insert("github_pages".into(), toml::Value::try_from(t).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?),
        None => deploy.remove("github_pages"),
    };
    Ok(())
}
fn config_json(settings: &MutableSiteSettings, server: &ServerConfig) -> serde_json::Value {
    let target = settings.deploy.github_pages.as_ref();
    serde_json::json!({
        "site": settings.site, "lobby": settings.lobby,
        "extensions": settings.extensions, "integrations": settings.integrations,
        "server":{"host":server.host,"port":server.port,"data_dir":server.data_dir},
        "deploy":{"github_pages":target.map(|t| serde_json::json!({
            "owner":t.owner,"repo":t.repo,"branch":t.branch,
            "pages_url":t.pages_url(),"base_path":t.base_path()
        }))}
    })
}
```

In the foundation `config_put`, while holding `config_write_lock`, reread the current TOML, apply existing allowlisted patches plus `apply_deploy_patch`, deserialize `Config`, validate its target, write a same-directory temp file, atomically rename, then update only the mutable snapshot:

```rust
apply_deploy_patch(&mut doc, update.deploy)?;
let serialized = toml::to_string_pretty(&doc).map_err(internal)?;
let parsed = Config::from_toml_str(&serialized).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
if let Some(target) = &parsed.deploy.github_pages { target.validate().map_err(bad_request)?; }
let tmp = toml_path.with_extension("toml.tmp");
tokio::fs::write(&tmp, serialized).await.map_err(internal)?;
tokio::fs::rename(&tmp, &toml_path).await.map_err(internal)?;
let next = MutableSiteSettings::from_config(&parsed);
*ctx.settings.write().await = next.clone();
Ok(Json(ConfigResponse { data: config_json(&next, &ctx.startup_server) }))
```

`config_get` must similarly read `let settings = ctx.settings.read().await;` and call `config_json(&settings, &ctx.startup_server)`. The removed full-config field must not be reintroduced.

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p oxipage-console --test config_deploy`

Expected: PASS; invalid targets do not write and server/custom keys survive.

- [ ] **Step 5: Commit**

```bash
git add crates/oxipage-console/src/per_site.rs crates/oxipage-console/tests/config_deploy.rs
git commit -m "feat(console): expose mutable GitHub Pages settings"
```

---

### Task 3: Safe repository-scoped deploy core

**Files:**
- Modify: `crates/oxipage-deploy/Cargo.toml`
- Rewrite: `crates/oxipage-deploy/src/lib.rs`
- Create: `crates/oxipage-deploy/tests/github_pages.rs`

**Interfaces:**
- Consumes: `GitHubPagesTarget`, `BuildManifest`
- Produces: specified `deploy_github_pages` signature, `DeployOutcome::{Deployed,Unchanged}`, typed events/errors

- [ ] **Step 1: Write failing precondition/origin tests**

```rust
use oxipage_core::{build_manifest::BuildManifest, site_paths::GitHubPagesTarget};
use oxipage_deploy::{deploy_github_pages, origin_matches, DeployError};
use tempfile::TempDir;
#[test]
fn manifest_mismatch_precedes_git_changes() {
    let repo=TempDir::new().unwrap(); let out=TempDir::new().unwrap();
    std::fs::write(out.path().join("index.html"), "ok").unwrap();
    let target=GitHubPagesTarget{owner:"owner".into(),repo:"site".into(),branch:"gh-pages".into()};
    let manifest=BuildManifest{build_id:"b1".into(),deployment_base:"/wrong/".into(),theme_id:"paper".into(),asset_revision:"a".into(),built_at:"2026-07-31T00:00:00Z".into()};
    let (tx,_)=tokio::sync::mpsc::channel(8);
    assert!(matches!(deploy_github_pages(repo.path(),out.path(),&target,&manifest,&tx), Err(DeployError::ManifestBaseMismatch{..})));
    assert!(!repo.path().join(".git").exists());
}
#[test]
fn origin_matching_is_exact() {
    let t=GitHubPagesTarget{owner:"owner".into(),repo:"site".into(),branch:"gh-pages".into()};
    assert!(origin_matches("https://github.com/owner/site.git",&t));
    assert!(origin_matches("git@github.com:owner/site.git",&t));
    assert!(!origin_matches("https://github.com/other/site.git",&t));
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p oxipage-deploy --test github_pages`

Expected: FAIL on the new API.

- [ ] **Step 3: Implement the contracts, filesystem operations, and cleanup guard**

Add `oxipage-core` and `uuid.workspace = true` to the deploy crate. Rewrite its public types:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeployOutcome { Deployed{url:String,commit:String}, Unchanged{url:String,commit:String} }
#[derive(Debug, thiserror::Error)]
pub enum DeployError {
 #[error("build output missing")] OutDirMissing,
 #[error("manifest base mismatch: expected {expected}, got {actual}")] ManifestBaseMismatch{expected:String,actual:String},
 #[error("invalid target: {0}")] InvalidTarget(String),
 #[error("gh not installed")] GhNotFound,
 #[error("gh authentication required")] NotAuthenticated,
 #[error("not a git repository")] NotGitRepository,
 #[error("origin mismatch")] OriginMismatch,
 #[error("git failed during {0}")] Git(&'static str),
 #[error(transparent)] Io(#[from] std::io::Error),
}
#[derive(Debug,Clone,serde::Serialize)]
#[serde(rename_all="snake_case",tag="event")]
pub enum DeployEvent {
 PreflightStarted,GhReady,AuthReady,RepositoryReady,WorktreeReady,
 FilesCopied{count:usize},CommitCreated{commit:String},Pushing{branch:String},
 Deployed{url:String,commit:String},Unchanged{url:String,commit:String},Failed{code:String,error:String},
}
pub fn origin_matches(remote:&str,target:&GitHubPagesTarget)->bool {
 let r=remote.trim().trim_end_matches('/').trim_end_matches(".git");
 r==format!("https://github.com/{}/{}",target.owner,target.repo)
 || r==format!("git@github.com:{}/{}",target.owner,target.repo)
 || r==format!("ssh://git@github.com/{}/{}",target.owner,target.repo)
}
```

Use Rust filesystem APIs and RAII:

```rust
fn copy_tree(src:&Path,dst:&Path)->std::io::Result<usize>{
 let mut n=0; for e in std::fs::read_dir(src)?{let e=e?;let from=e.path();let to=dst.join(e.file_name());
 if from.is_dir(){std::fs::create_dir_all(&to)?;n+=copy_tree(&from,&to)?;}else{std::fs::copy(from,to)?;n+=1;}} Ok(n)
}
fn clear(dir:&Path)->std::io::Result<()> { for e in std::fs::read_dir(dir)? { let p=e?.path(); if p.file_name().is_some_and(|n|n==".git"){continue;} if p.is_dir(){std::fs::remove_dir_all(p)?}else{std::fs::remove_file(p)?} } Ok(()) }
struct Cleanup{repo:PathBuf,worktree:PathBuf}
impl Drop for Cleanup{fn drop(&mut self){
 let _=Command::new("git").current_dir(&self.repo).args(["worktree","remove","--force"]).arg(&self.worktree).output();
 let _=Command::new("git").current_dir(&self.repo).args(["worktree","prune"]).output();
 let _=std::fs::remove_dir_all(&self.worktree);
}}
fn run(cwd:&Path,program:&str,args:&[&str],step:&'static str)->Result<Output,DeployError>{
 let o=Command::new(program).current_dir(cwd).args(args).output().map_err(|e|if program=="gh"&&e.kind()==std::io::ErrorKind::NotFound{DeployError::GhNotFound}else{DeployError::Io(e)})?;
 if o.status.success(){Ok(o)}else{Err(DeployError::Git(step))}
}
```

Implement the required entry point:

```rust
pub fn deploy_github_pages(repo_dir:&Path,out_dir:&Path,target:&GitHubPagesTarget,manifest:&BuildManifest,tx:&mpsc::Sender<DeployEvent>)->Result<DeployOutcome,DeployError>{
 target.validate().map_err(|e|DeployError::InvalidTarget(e.to_string()))?;
 if !out_dir.join("index.html").is_file(){return Err(DeployError::OutDirMissing)}
 let expected=target.base_path(); if manifest.deployment_base!=expected{return Err(DeployError::ManifestBaseMismatch{expected,actual:manifest.deployment_base.clone()})}
 let _=tx.blocking_send(DeployEvent::PreflightStarted);
 run(repo_dir,"gh",&["--version"],"gh version")?; let _=tx.blocking_send(DeployEvent::GhReady);
 if run(repo_dir,"gh",&["auth","status"],"gh auth").is_err(){return Err(DeployError::NotAuthenticated)} let _=tx.blocking_send(DeployEvent::AuthReady);
 run(repo_dir,"git",&["rev-parse","--is-inside-work-tree"],"repository").map_err(|_|DeployError::NotGitRepository)?;
 let remote=run(repo_dir,"git",&["remote","get-url","origin"],"origin")?;
 if !origin_matches(&String::from_utf8_lossy(&remote.stdout),target){return Err(DeployError::OriginMismatch)} let _=tx.blocking_send(DeployEvent::RepositoryReady);
 let work=std::env::temp_dir().join(format!("oxipage-deploy-{}",uuid::Uuid::new_v4())); let w=work.to_string_lossy().into_owned();
 let remote_ref=format!("refs/remotes/origin/{}",target.branch);
 let exists=Command::new("git").current_dir(repo_dir).args(["show-ref","--verify","--quiet",&remote_ref]).status().is_ok_and(|s|s.success());
 if exists{run(repo_dir,"git",&["worktree","add","--detach",&w,&remote_ref],"worktree")?;}else{run(repo_dir,"git",&["worktree","add","--detach",&w],"worktree")?;}
 let cleanup=Cleanup{repo:repo_dir.into(),worktree:work.clone()}; let _=tx.blocking_send(DeployEvent::WorktreeReady);
 clear(&work)?; let count=copy_tree(out_dir,&work)?; let _=tx.blocking_send(DeployEvent::FilesCopied{count}); run(&work,"git",&["add","-A"],"add")?;
 let changed=!Command::new("git").current_dir(&work).args(["diff","--cached","--quiet"]).status()?.success(); let url=target.pages_url();
 if !changed{let o=run(&work,"git",&["rev-parse","HEAD"],"head")?;let commit=String::from_utf8_lossy(&o.stdout).trim().into();let result=DeployOutcome::Unchanged{url:url.clone(),commit:commit.clone()};let _=tx.blocking_send(DeployEvent::Unchanged{url,commit});drop(cleanup);return Ok(result)}
 let msg=format!("deploy: {}",manifest.build_id);run(&work,"git",&["commit","-m",&msg],"commit")?;let o=run(&work,"git",&["rev-parse","HEAD"],"head")?;let commit=String::from_utf8_lossy(&o.stdout).trim().to_string();let _=tx.blocking_send(DeployEvent::CommitCreated{commit:commit.clone()});
 let push=format!("HEAD:refs/heads/{}",target.branch);let _=tx.blocking_send(DeployEvent::Pushing{branch:target.branch.clone()});run(&work,"git",&["push","origin",&push],"push")?;
 let result=DeployOutcome::Deployed{url:url.clone(),commit:commit.clone()};let _=tx.blocking_send(DeployEvent::Deployed{url,commit});drop(cleanup);Ok(result)
}
```

- [ ] **Step 4: Verify tests and forbidden-command scan**

Run: `cargo test -p oxipage-deploy --test github_pages`

Expected: PASS.

Run: `grep -n 'bash\|Command::new("cp")\|Command::new("rm")' crates/oxipage-deploy/src/lib.rs`

Expected: no matches.

- [ ] **Step 5: Commit**

```bash
git add crates/oxipage-deploy
git commit -m "refactor(deploy): make GitHub Pages deployment repository scoped"
```

---

### Task 4: One operation guard for build and deploy

**Files:**
- Create: `crates/oxipage-console/src/operations.rs`
- Modify: `crates/oxipage-console/src/{lib.rs,loader.rs,sites_runtime.rs}`
- Modify: `crates/oxipage-console/src/build/build_run.rs`
- Modify: `crates/oxipage-console/src/deploy/deploy_run.rs`
- Create: `crates/oxipage-console/tests/operations.rs`

**Interfaces:**
- Produces: `SiteOperationGuard::{try_start,current,subscribe,publish,finish}` and retained `OperationSnapshot`

- [ ] **Step 1: Write failing exclusion/retention tests**

```rust
use oxipage_console::operations::{OperationEvent,SiteOperationGuard,SiteOperationKind};
#[test] fn conflicts_only_within_site(){let g=SiteOperationGuard::new();g.try_start("a","b1",SiteOperationKind::Build).unwrap();let e=g.try_start("a","d1",SiteOperationKind::Deploy).unwrap_err();assert_eq!((e.kind,e.run_id),(SiteOperationKind::Build,"b1".into()));assert!(g.try_start("b","d2",SiteOperationKind::Deploy).is_ok());}
#[test] fn terminal_state_survives_finish(){let g=SiteOperationGuard::new();g.try_start("a","d1",SiteOperationKind::Deploy).unwrap();g.publish("a",OperationEvent::terminal("deployed",serde_json::json!({"url":"u"}))).unwrap();g.finish("a").unwrap();let s=g.current("a").unwrap();assert!(!s.active);assert_eq!(s.terminal.unwrap()["url"],"u");}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p oxipage-console --test operations`

Expected: FAIL because `operations` is absent.

- [ ] **Step 3: Implement and wire the shared guard**

```rust
// operations.rs
#[derive(Debug,Clone,Copy,Serialize,PartialEq,Eq)]#[serde(rename_all="snake_case")]pub enum SiteOperationKind{Build,Deploy}
#[derive(Debug,Clone,Serialize)]pub struct OperationEvent{pub event:String,pub data:Value,pub terminal:bool}
impl OperationEvent{pub fn progress(e:impl Into<String>,data:Value)->Self{Self{event:e.into(),data,terminal:false}}pub fn terminal(e:impl Into<String>,data:Value)->Self{Self{event:e.into(),data,terminal:true}}}
#[derive(Debug,Clone,Serialize)]pub struct OperationSnapshot{pub kind:SiteOperationKind,pub run_id:String,pub active:bool,pub started_at:String,pub terminal:Option<Value>}
#[derive(Debug,PartialEq,Eq)]pub struct OperationConflict{pub kind:SiteOperationKind,pub run_id:String}
struct Slot{snapshot:OperationSnapshot,tx:tokio::sync::broadcast::Sender<OperationEvent>}
pub struct SiteOperationGuard{slots:dashmap::DashMap<String,Slot>}
impl SiteOperationGuard{
 pub fn new()->Self{Self{slots:dashmap::DashMap::new()}}
 pub fn try_start(&self,slug:&str,id:&str,kind:SiteOperationKind)->Result<(),OperationConflict>{use dashmap::mapref::entry::Entry;match self.slots.entry(slug.into()){Entry::Occupied(o)if o.get().snapshot.active=>Err(OperationConflict{kind:o.get().snapshot.kind,run_id:o.get().snapshot.run_id.clone()}),Entry::Occupied(mut o)=>{let(tx,_)=tokio::sync::broadcast::channel(128);o.insert(Slot{snapshot:OperationSnapshot{kind,run_id:id.into(),active:true,started_at:now(),terminal:None},tx});Ok(())},Entry::Vacant(v)=>{let(tx,_)=tokio::sync::broadcast::channel(128);v.insert(Slot{snapshot:OperationSnapshot{kind,run_id:id.into(),active:true,started_at:now(),terminal:None},tx});Ok(())}}}
 pub fn current(&self,s:&str)->Option<OperationSnapshot>{self.slots.get(s).map(|x|x.snapshot.clone())}
 pub fn subscribe(&self,s:&str,id:&str)->Option<tokio::sync::broadcast::Receiver<OperationEvent>>{self.slots.get(s).filter(|x|x.snapshot.run_id==id).map(|x|x.tx.subscribe())}
 pub fn publish(&self,s:&str,e:OperationEvent)->Result<(),()>{let mut x=self.slots.get_mut(s).ok_or(())?;if e.terminal{x.snapshot.terminal=Some(e.data.clone())}let _=x.tx.send(e);Ok(())}
 pub fn finish(&self,s:&str)->Result<(),()>{self.slots.get_mut(s).ok_or(())?.snapshot.active=false;Ok(())}
}
```

Export the module. Replace `build_guard` and `deploy_guard` everywhere with one `operation_guard: Arc<SiteOperationGuard>`, created once in `lib.rs` and cloned through `SiteRegistry`/`SiteLoader` into every `SiteContext`. Build/deploy POST conflict bodies become:

```rust
Json(serde_json::json!({"error":"site_operation_in_progress","kind":conflict.kind,"run_id":conflict.run_id}))
```

Relay build events through `OperationEvent`, publish a terminal event before `finish`, and remove old guard structs/exports.

- [ ] **Step 4: Run focused tests**

Run: `cargo test -p oxipage-console --test operations`

Expected: PASS.

Run: `cargo test -p oxipage-console --test build_deploy_preview`

Expected: PASS after assertions use the common 409 shape.

- [ ] **Step 5: Commit**

```bash
git add crates/oxipage-console/src crates/oxipage-console/tests/operations.rs crates/oxipage-console/tests/build_deploy_preview.rs
git commit -m "refactor(console): serialize site build and deploy operations"
```

---

### Task 5: Deploy history and run lifecycle

**Files:**
- Create: `crates/oxipage-core/migrations/core/0007_deploy_log.sql`
- Rewrite: `crates/oxipage-console/src/deploy/deploy_run.rs`
- Modify: `crates/oxipage-console/src/per_site.rs`
- Create: `crates/oxipage-console/tests/deploy_api.rs`

**Interfaces:**
- Consumes: shared guard, live target snapshot, manifest, deploy outcome
- Produces: durable `running|deployed|unchanged|failed` records and `GET /deploys`

- [ ] **Step 1: Write the failing history test**

```rust
#[tokio::test] async fn deploys_are_newest_first_and_limited(){
 let f=site_router().await;insert_deploy(&f.db,"r1","deployed").await;insert_deploy(&f.db,"r2","unchanged").await;
 let j=get_json(f.app,"/s/blog/deploys?limit=1").await;assert_eq!(j["data"].as_array().unwrap().len(),1);assert_eq!(j["data"][0]["run_id"],"r2");
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p oxipage-console --test deploy_api`

Expected: FAIL because table/route are absent.

- [ ] **Step 3: Add schema, execution snapshots, and recorder**

```sql
CREATE TABLE deploy_log(id INTEGER PRIMARY KEY AUTOINCREMENT,run_id TEXT NOT NULL UNIQUE,build_id TEXT NOT NULL,target TEXT NOT NULL,owner TEXT NOT NULL,repo TEXT NOT NULL,branch TEXT NOT NULL,base_path TEXT NOT NULL,status TEXT NOT NULL CHECK(status IN('running','deployed','unchanged','failed')),url TEXT,commit_sha TEXT,error_code TEXT,error TEXT,started_at TEXT NOT NULL,finished_at TEXT);
CREATE INDEX deploy_log_started_at_idx ON deploy_log(started_at DESC);
```

```rust
pub struct DeployRun{pub run_id:String,pub repo_dir:PathBuf,pub out_dir:PathBuf,pub target:GitHubPagesTarget,pub manifest:BuildManifest,pub db:SqlitePool,pub slug:String,pub guard:Arc<SiteOperationGuard>}
fn normalize(e:&DeployError)->(&'static str,&'static str){match e{DeployError::GhNotFound=>("gh_not_installed","Install GitHub CLI"),DeployError::NotAuthenticated=>("gh_auth_required","Run gh auth login"),DeployError::OriginMismatch=>("origin_mismatch","Git origin does not match configuration"),DeployError::ManifestBaseMismatch{..}=>("stale_build_base","Rebuild for this Pages base"),DeployError::OutDirMissing=>("build_required","Build before deploying"),_=>("deploy_failed","Deployment failed; inspect the live log")}}
```

Insert `running` before spawn. Call the Task 3 function in `spawn_blocking`, relay all events, and update terminal state:

```rust
let result=tokio::task::spawn_blocking(move||deploy_github_pages(&repo,&out,&target,&manifest,&tx)).await;
let(status,url,commit,code,error)=match result{Ok(Ok(DeployOutcome::Deployed{url,commit}))=>("deployed",Some(url),Some(commit),None,None),Ok(Ok(DeployOutcome::Unchanged{url,commit}))=>("unchanged",Some(url),Some(commit),None,None),Ok(Err(e))=>{let(c,m)=normalize(&e);("failed",None,None,Some(c),Some(m))},Err(_)=>("failed",None,None,Some("deploy_panicked"),Some("Deployment worker stopped"))};
sqlx::query("UPDATE deploy_log SET status=?2,url=?3,commit_sha=?4,error_code=?5,error=?6,finished_at=datetime('now') WHERE run_id=?1").bind(&id).bind(status).bind(&url).bind(&commit).bind(code).bind(error).execute(&db).await?;
guard.publish(&slug,OperationEvent::terminal(status,serde_json::json!({"status":status,"url":url,"commit":commit,"error_code":code,"error":error}))).ok();guard.finish(&slug).ok();
```

Add `DeployRecord` (`#[derive(Serialize,sqlx::FromRow)]`) with every migration column and:

```rust
async fn deploys_list(Extension(ctx):Extension<Arc<SiteContext>>,Query(q):Query<RecentQuery>)->Result<Json<Value>,ApiError>{let rows=sqlx::query_as::<_,DeployRecord>("SELECT * FROM deploy_log ORDER BY id DESC LIMIT ?1").bind(q.limit.unwrap_or(50).clamp(1,100)).fetch_all(&ctx.db).await?;Ok(Json(json!({"data":rows})))}
```

Register `.route("/deploys", get(deploys_list))`.

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p oxipage-console --test deploy_api`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/oxipage-core/migrations/core/0007_deploy_log.sql crates/oxipage-console/src/deploy/deploy_run.rs crates/oxipage-console/src/per_site.rs crates/oxipage-console/tests/deploy_api.rs
git commit -m "feat(console): persist GitHub Pages deploy history"
```

---

### Task 6: Preflight and current-operation APIs

**Files:**
- Modify: `crates/oxipage-console/src/per_site.rs`
- Modify: `crates/oxipage-console/tests/deploy_api.rs`

**Interfaces:**
- Produces: `GET /deploy/preflight`, `GET /operations/current`; deploy POST preflight gate

- [ ] **Step 1: Write failing stale/current tests**

```rust
#[tokio::test] async fn preflight_reports_stale_base_and_theme(){let f=configured_site("owner","site").await;write_manifest(&f.out_dir,"/wrong/","midnight");let j=get_json(f.app,"/s/blog/deploy/preflight").await;let codes:Vec<_>=j["data"]["problems"].as_array().unwrap().iter().map(|p|p["code"].as_str().unwrap()).collect();assert!(codes.contains(&"stale_build_base"));assert!(codes.contains(&"stale_build_theme"));}
#[tokio::test] async fn current_returns_common_run(){let f=site_router().await;f.guard.try_start("blog","b7",SiteOperationKind::Build).unwrap();let j=get_json(f.app,"/s/blog/operations/current").await;assert_eq!(j["data"]["run_id"],"b7");assert_eq!(j["data"]["kind"],"build");}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p oxipage-console --test deploy_api`

Expected: FAIL with 404.

- [ ] **Step 3: Implement all checks and routes**

```rust
#[derive(Serialize)]struct PreflightProblem{code:&'static str,message:String,action:Option<&'static str>}
#[derive(Serialize)]struct DeployPreflight{configured:bool,gh_installed:bool,authenticated:bool,git_repository:bool,origin_matches:bool,build_compatible:bool,pages_url:Option<String>,base_path:Option<String>,problems:Vec<PreflightProblem>}
async fn evaluate_preflight(ctx:&SiteContext)->DeployPreflight{
 let settings=ctx.settings.read().await.clone(); let target=settings.deploy.github_pages.filter(|t|t.validate().is_ok()); let mut p=Vec::new();
 if target.is_none(){p.push(problem("deploy_not_configured","Configure GitHub Pages",Some("open_settings")))}
 let gh=Command::new("gh").current_dir(&ctx.project_dir).arg("--version").output().is_ok_and(|o|o.status.success());if !gh{p.push(problem("gh_not_installed","Install GitHub CLI",Some("install_gh")))}
 let auth=gh&&Command::new("gh").current_dir(&ctx.project_dir).args(["auth","status"]).output().is_ok_and(|o|o.status.success());if gh&&!auth{p.push(problem("gh_auth_required","Run gh auth login",Some("authenticate_gh")))}
 let git=Command::new("git").current_dir(&ctx.project_dir).args(["rev-parse","--is-inside-work-tree"]).output().is_ok_and(|o|o.status.success());if !git{p.push(problem("not_git_repository","Selected site is not a Git repository",None))}
 let origin=match(&target,git){(Some(t),true)=>Command::new("git").current_dir(&ctx.project_dir).args(["remote","get-url","origin"]).output().ok().filter(|o|o.status.success()).is_some_and(|o|origin_matches(&String::from_utf8_lossy(&o.stdout),t)),_=>false};if target.is_some()&&git&&!origin{p.push(problem("origin_mismatch","Origin does not match configuration",Some("open_settings")))}
 let manifest=BuildManifest::read_from(&ctx.out_dir).ok();if manifest.is_none(){p.push(problem("build_required","Build the site",Some("build")))}
 let theme:String=sqlx::query_scalar("SELECT theme_id FROM theme_config WHERE id=1").fetch_optional(&ctx.db).await.ok().flatten().unwrap_or_else(||"paper".into());
 if let(Some(t),Some(m))=(&target,&manifest){if m.deployment_base!=t.base_path(){p.push(problem("stale_build_base","Rebuild for the Pages base",Some("rebuild")))}if m.theme_id!=theme{p.push(problem("stale_build_theme","Rebuild for the current theme",Some("rebuild")))}}
 if !ctx.out_dir.join("index.html").is_file(){p.push(problem("missing_index","Build index is missing",Some("rebuild")))}
 if manifest.is_some()&&!ctx.out_dir.join("assets").is_dir(){p.push(problem("missing_assets","Referenced assets are missing",Some("rebuild")))}
 let compatible=p.is_empty();DeployPreflight{configured:target.is_some(),gh_installed:gh,authenticated:auth,git_repository:git,origin_matches:origin,build_compatible:compatible,pages_url:target.as_ref().map(|t|t.pages_url()),base_path:target.as_ref().map(|t|t.base_path()),problems:p}
}
```

```rust
async fn deploy_preflight(Extension(ctx):Extension<Arc<SiteContext>>)->Json<Value>{Json(json!({"data":evaluate_preflight(&ctx).await}))}
async fn operation_current(Extension(ctx):Extension<Arc<SiteContext>>)->Json<Value>{Json(json!({"data":ctx.operation_guard.current(&ctx.slug)}))}
```

Register both routes. `deploy_post` calls `evaluate_preflight`; on failure return 424 with problems. Then read target from a fresh `ctx.settings.read().await.deploy.github_pages.clone()`, read the manifest once, and place both snapshots plus `ctx.project_dir/out_dir` in `DeployRun`. Return `{data:{run_id,kind:"deploy",status:"queued"}}`.

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p oxipage-console --test deploy_api`

Expected: PASS for all nine checks, current operation, and 424 behavior.

- [ ] **Step 5: Commit**

```bash
git add crates/oxipage-console/src/per_site.rs crates/oxipage-console/tests/deploy_api.rs
git commit -m "feat(console): add deploy preflight and reconnect APIs"
```

---

### Task 7: CLI registered-site convergence

**Files:**
- Rewrite: `crates/oxipage-cli/src/commands/deploy.rs`
- Create: `crates/oxipage-cli/tests/deploy_site.rs`

**Interfaces:**
- Produces: `resolve_deploy_project`; `--site`/default/legacy precedence; shared-core invocation

- [ ] **Step 1: Write failing resolution tests**

```rust
#[test]fn explicit_site_wins(){assert_eq!(resolve_deploy_project(Some("beta"),&registry(),None).unwrap(),PathBuf::from("/sites/beta"));}
#[test]fn default_is_used(){assert_eq!(resolve_deploy_project(None,&registry(),None).unwrap(),PathBuf::from("/sites/alpha"));}
#[test]fn legacy_only_without_registry(){assert_eq!(resolve_deploy_project(None,&SitesFile::default(),Some(Path::new("/legacy/oxipage.toml"))).unwrap(),PathBuf::from("/legacy"));assert!(resolve_deploy_project(Some("missing"),&registry(),Some(Path::new("/legacy/oxipage.toml"))).is_err());}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p oxipage --test deploy_site`

Expected: FAIL.

- [ ] **Step 3: Implement resolution and shared-core call**

```rust
pub fn resolve_deploy_project(requested:Option<&str>,sites:&SitesFile,legacy:Option<&Path>)->anyhow::Result<PathBuf>{
 if !sites.sites.is_empty(){let name=sites.resolve_name(requested).ok_or_else(||anyhow!("select a site with --site or set a default"))?;return sites.sites.get(&name).map(|e|e.path.clone()).ok_or_else(||anyhow!("site '{name}' is not registered"));}
 legacy.and_then(Path::parent).map(Path::to_path_buf).ok_or_else(||anyhow!("no registered site and no oxipage.toml"))
}
```

```rust
let sites=crate::sites::load_sites();let legacy=std::env::var_os("OXIPAGE_CONFIG").map(PathBuf::from).or_else(||Path::new("oxipage.toml").exists().then(||PathBuf::from("oxipage.toml")));
let project=resolve_deploy_project(c.site.as_deref(),&sites,legacy.as_deref())?;let cfg=Config::load(&project.join("oxipage.toml"))?;
let target=cfg.deploy.github_pages.ok_or_else(||anyhow!("[deploy.github_pages] is not configured"))?;target.validate()?;
let data=if cfg.server.data_dir.is_absolute(){cfg.server.data_dir}else{project.join(cfg.server.data_dir)};let out_dir=data.join("out");let manifest=BuildManifest::read_from(&out_dir)?;
let(tx,mut rx)=tokio::sync::mpsc::channel(64);let repo=project.clone();let target2=target.clone();let handle=tokio::task::spawn_blocking(move||deploy_github_pages(&repo,&out_dir,&target2,&manifest,&tx));while let Some(e)=rx.recv().await{out.ok(deploy_event_label(&e))?;}match handle.await??{DeployOutcome::Deployed{url,commit}=>out.ok(format!("deployed {commit} to {url}")),DeployOutcome::Unchanged{url,commit}=>out.ok(format!("unchanged at {commit}: {url}"))}
```

Retain dry-run, but print selected `out_dir` and `target.pages_url()`. Update `deploy_event_label` exhaustively for every Task 3 event.

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p oxipage --test deploy_site`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/oxipage-cli/src/commands/deploy.rs crates/oxipage-cli/tests/deploy_site.rs
git commit -m "fix(cli): honor registered site for deploy"
```

---

### Task 8: Typed Admin APIs and Deploy page

**Files:**
- Modify: `web/src/admin/shared/api.ts`
- Rewrite: `web/src/admin/deploy/DeployPage.tsx`

**Interfaces:**
- Consumes: Tasks 2, 5, 6 APIs and preview subproject route
- Produces: preflight, Preview Site, history, reattachment, stale badge

- [ ] **Step 1: Add exact client contracts**

```typescript
export interface GitHubPagesTarget{owner:string;repo:string;branch:string;pages_url?:string;base_path?:string}
export interface DeployPreflight{configured:boolean;gh_installed:boolean;authenticated:boolean;git_repository:boolean;origin_matches:boolean;build_compatible:boolean;pages_url:string|null;base_path:string|null;problems:{code:string;message:string;action:string|null}[]}
export interface DeployRecord{id:number;run_id:string;build_id:string;target:string;owner:string;repo:string;branch:string;base_path:string;status:"running"|"deployed"|"unchanged"|"failed";url:string|null;commit_sha:string|null;error_code:string|null;error:string|null;started_at:string;finished_at:string|null}
export interface CurrentOperation{kind:"build"|"deploy";run_id:string;active:boolean;started_at:string;terminal:Record<string,unknown>|null}
export class OperationConflictError extends Error{constructor(public kind:"build"|"deploy",public id:string){super("site_operation_in_progress")}}
async function start(slug:string,path:"/build"|"/deploy"){const r=await siteScopedFetch(slug,path,{method:"POST"});if(r.status===409){const b=await r.json();throw new OperationConflictError(b.kind,b.run_id)}return jsonOrThrow<{data:{run_id:string;kind:"build"|"deploy";status:string}}>(r)}
export const triggerBuild=(s:string)=>start(s,"/build");export const triggerDeploy=(s:string)=>start(s,"/deploy");
export async function getDeployPreflight(s:string){return(await jsonOrThrow<{data:DeployPreflight}>(await siteScopedFetch(s,"/deploy/preflight"))).data}
export async function listDeploys(s:string){return(await jsonOrThrow<{data:DeployRecord[]}>(await siteScopedFetch(s,"/deploys?limit=50"))).data}
export async function getCurrentOperation(s:string){return(await jsonOrThrow<{data:CurrentOperation|null}>(await siteScopedFetch(s,"/operations/current"))).data}
export const operationStreamUrl=(s:string,k:"build"|"deploy",id:string)=>`${CONSOLE_BASE}/s/${encodeURIComponent(s)}/${k}/${encodeURIComponent(id)}/stream`;
export const previewSiteUrl=(s:string)=>`${CONSOLE_BASE}/preview/${encodeURIComponent(s)}/`;
```

Extend `ConfigResponse` with `deploy:{github_pages:GitHubPagesTarget|null}`, `updateConfig` with a deploy patch, and `StatsResponse` with `last_deploy: DeployRecord|null`.

- [ ] **Step 2: Run TypeScript to expose old call sites**

Run: `cd web && npx tsc --noEmit`

Expected: FAIL in DeployPage old IDs/stream helpers.

- [ ] **Step 3: Rewrite DeployPage behavior and sections**

```typescript
const buildsQ=useQuery({queryKey:["site",slug,"builds"],queryFn:()=>listBuilds(slug!),enabled:!!slug});
const deploysQ=useQuery({queryKey:["site",slug,"deploys"],queryFn:()=>listDeploys(slug!),enabled:!!slug});
const preflightQ=useQuery({queryKey:["site",slug,"deploy-preflight"],queryFn:()=>getDeployPreflight(slug!),enabled:!!slug,refetchInterval:15000});
const currentQ=useQuery({queryKey:["site",slug,"operation"],queryFn:()=>getCurrentOperation(slug!),enabled:!!slug});
useEffect(()=>{if(currentQ.data?.active&&!esRef.current)attachStream(currentQ.data.kind,currentQ.data.run_id)},[currentQ.data?.run_id,currentQ.data?.active]);
const stale=preflightQ.data?.problems.some(p=>p.code==="stale_build_base"||p.code==="stale_build_theme")??false;
const hasBuild=!preflightQ.data?.problems.some(p=>p.code==="build_required"||p.code==="missing_index");
```

Both action catches attach to `error.kind/error.id`, regardless of requested operation. Use `operationStreamUrl`; treat `unchanged` as terminal and invalidate builds/deploys/preflight/current/stats. Render header actions:

```tsx
<Button variant="outline" onClick={onBuild} disabled={busy}>Build</Button>
<Button variant="outline" asChild disabled={!hasBuild||stale}>{hasBuild&&!stale?<a href={previewSiteUrl(slug!)} target="_blank" rel="noreferrer">Preview Site ↗</a>:<span>Preview Site ↗</span>}</Button>
{stale&&<Badge variant="warning">Stale build</Badge>}
<Button onClick={onDeploy} disabled={busy||!preflightQ.data?.build_compatible}>Deploy</Button>
```

Render the preflight card and remediation:

```tsx
<section className="rounded-lg border border-line p-5"><h2>Deployment preflight</h2>
{preflightQ.data?.problems.map(p=><div key={p.code} className="flex justify-between text-sm"><span>{p.message}</span>{(p.action==="build"||p.action==="rebuild")&&<Button onClick={onBuild}>Build</Button>}{p.action==="open_settings"&&<Button onClick={()=>navigate(`/sites/${slug}/settings`)}>Settings</Button>}</div>)}
{preflightQ.data?.pages_url&&<a href={preflightQ.data.pages_url} target="_blank" rel="noreferrer">{preflightQ.data.pages_url}</a>}</section>
```

Render last deployment and history from real records:

```tsx
{deploysQ.data?.[0]&&<section><Badge>{deploysQ.data[0].status}</Badge>{deploysQ.data[0].url&&<a href={deploysQ.data[0].url!} target="_blank" rel="noreferrer">Open site ↗</a>}</section>}
<div>{deploysQ.data?.map(d=><div key={d.run_id} className="grid grid-cols-4"><span>{d.owner}/{d.repo}</span><span>{d.status}</span><code>{d.commit_sha?.slice(0,8)??"—"}</code><span>{d.started_at}</span></div>)}</div>
```

Keep build history; remove the hard-coded Build → Generate → Deploy rail.

- [ ] **Step 4: Typecheck and smoke**

Run: `cd web && npx tsc --noEmit`

Expected: PASS.

Manual smoke: open Deploy, verify preflight/preview; refresh during a run and observe reattachment; confirm stale build disables preview/deploy; confirm terminal result refreshes both histories.

- [ ] **Step 5: Commit**

```bash
git add web/src/admin/shared/api.ts web/src/admin/deploy/DeployPage.tsx
git commit -m "feat(admin): add preflighted reconnectable deployment"
```

---

### Task 9: Deployment settings, dashboard status, and final verification

**Files:**
- Modify: `web/src/admin/settings/SettingsPage.tsx`
- Modify: `crates/oxipage-console/src/per_site.rs`
- Modify: `web/src/admin/dashboard/DashboardPage.tsx`
- Modify: `crates/oxipage-console/tests/deploy_api.rs`

**Interfaces:**
- Produces: Settings deployment section, explicit base URL action, `stats.last_deploy`

- [ ] **Step 1: Write the failing last-deploy stats test**

```rust
#[tokio::test]async fn stats_uses_latest_deploy(){let f=site_router().await;insert_deploy(&f.db,"old","deployed").await;insert_deploy(&f.db,"new","unchanged").await;let j=get_json(f.app,"/s/blog/stats").await;assert_eq!(j["data"]["last_deploy"]["run_id"],"new");}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p oxipage-console --test deploy_api`

Expected: FAIL because stats lacks `last_deploy`.

- [ ] **Step 3: Add Settings state, validation, save, and UI**

```typescript
const[owner,setOwner]=useState("");const[repo,setRepo]=useState("");const[branch,setBranch]=useState("gh-pages");
const component=/^[A-Za-z0-9._-]+$/;const branchOk=/^[A-Za-z0-9._/-]+$/.test(branch)&&!branch.includes("..")&&!branch.startsWith("/")&&!branch.endsWith("/");
const valid=component.test(owner)&&component.test(repo)&&branchOk;
const pagesUrl=valid?(repo.toLowerCase()===`${owner.toLowerCase()}.github.io`?`https://${owner}.github.io/`:`https://${owner}.github.io/${repo}/`):"";
const basePath=valid?(repo.toLowerCase()===`${owner.toLowerCase()}.github.io`?"/":`/${repo}/`):"";
```

Populate/reset from `data.deploy.github_pages`. Add to the existing save payload:

```typescript
deploy:{github_pages:valid?{owner,repo,branch}:null}
```

Render before Default Site:

```tsx
<section className="rounded-lg border border-line p-5"><h3>Deployment · GitHub Pages</h3><p className="text-xs text-muted">Authentication remains in GitHub CLI; no token is stored.</p>
<SettingsField label="Owner" value={owner} onChange={setOwner}/><SettingsField label="Repository" value={repo} onChange={setRepo}/><SettingsField label="Publish branch" value={branch} onChange={setBranch}/>
<div className="ml-28 text-xs">Pages URL: {pagesUrl||"—"}<br/>Base path: <code>{basePath||"—"}</code></div>
<Button disabled={!pagesUrl||baseUrl===pagesUrl} onClick={()=>setBaseUrl(pagesUrl)}>Use this Pages URL as Site Base URL</Button>
<Button variant="outline" onClick={()=>navigate(`/sites/${slug}/deploy`)}>Open Deploy page</Button></section>
```

Show field errors and disable Save for a partial invalid target. Do not reuse `integrations.github_username` as owner.

- [ ] **Step 4: Return real last deploy and update Dashboard**

```rust
let last_deploy=sqlx::query_as::<_,DeployRecord>("SELECT * FROM deploy_log ORDER BY id DESC LIMIT 1").fetch_optional(&ctx.db).await.map_err(internal)?;
// add to stats JSON
"last_deploy": last_deploy
```

```tsx
{stats?.last_deploy?<div><Badge variant={stats.last_deploy.status==="failed"?"warning":"positive"}>{stats.last_deploy.status}</Badge><span>{stats.last_deploy.owner}/{stats.last_deploy.repo}</span>{stats.last_deploy.url&&<a href={stats.last_deploy.url} target="_blank" rel="noreferrer">Open site ↗</a>}</div>:<p>No deployments yet.</p>}
```

- [ ] **Step 5: Run focused verification**

Run: `cargo test -p oxipage-core --test deploy_config`

Run: `cargo test -p oxipage-deploy --test github_pages`

Run: `cargo test -p oxipage-console --test config_deploy`

Run: `cargo test -p oxipage-console --test operations`

Run: `cargo test -p oxipage-console --test deploy_api`

Run: `cargo test -p oxipage --test deploy_site`

Run: `cd web && npx tsc --noEmit`

Expected: every command PASS.

- [ ] **Step 6: Perform end-to-end smoke**

Manual smoke with two temporary registered sites/remotes: confirm root `/` and project `/<repo>/`; launch console outside both repositories; deploy only the selected repo; verify origin/auth/base/theme failures modify no Git state; start deploy on site A and observe build/deploy 409 while site B remains operable; refresh and reattach; produce deployed, unchanged, and failed history rows; run `oxipage deploy --site <second-slug>` and confirm the second target receives output whose assets/data/media resolve under its Pages URL.

- [ ] **Step 7: Commit**

```bash
git add web/src/admin/settings/SettingsPage.tsx web/src/admin/dashboard/DashboardPage.tsx crates/oxipage-console/src/per_site.rs crates/oxipage-console/tests/deploy_api.rs
git commit -m "feat(console): expose GitHub Pages settings and status"
```

---

## Self-Review

- **Spec coverage:** Tasks 1–2 cover config scaffolding, deserialization, validation, URL/base derivation, ConfigUpdate/Response, atomic preservation, and explicit base URL behavior. Task 3 covers the required signature, explicit CWD, origin verification, UUID worktree, Rust copying/removal, cleanup guard, branch, manifest precondition, and outcomes. Tasks 4–6 cover one guard, common 409, retained terminal state, SSE reattach, deploy history, preflight checks, 424, and current/history routes. Task 7 honors `--site`, registry default, and legacy fallback. Tasks 8–9 cover Preview Site, preflight, histories, stale badge, Settings, dashboard, and end-to-end root/project verification.
- **Foundation consistency:** Plan code reads mutable deployment/site settings through `ctx.settings`, immutable server fields through `ctx.startup_server`, and paths through resolved `SiteContext` fields; the removed full-config field is never used.
- **Out of scope:** No custom domain, token storage, repository creation, rollback, scheduled deployment, or non-GitHub provider is added.
- **Placeholder scan:** Implementation steps contain concrete code; commands and manual checks are explicit; there are no deferred implementation markers or “similar to” instructions.
- **Type consistency:** `run_id`, `kind`, `DeployOutcome`, `DeployEvent`, `GitHubPagesTarget`, `BuildManifest`, `DeployRecord`, `DeployPreflight`, and the common `{error,kind,run_id}` conflict shape match end to end.
- **Safety:** Target values are validated before commands, commands use separate arguments and explicit directories, cleanup is RAII, config writes preserve unknown/server keys, and history stores normalized errors only.
