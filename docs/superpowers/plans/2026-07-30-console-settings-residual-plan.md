# Console Settings Residual — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Fix `set_default` stub to actually persist; defer Purge All Data.

**Architecture:** Mirrors the existing `remove_site` write pattern (sites.toml read→modify→write→sync). Validates slug before persisting.

**Tech Stack:** Rust (axum, sqlx, toml), TypeScript/React

## Global Constraints

- set_default validates slug is registered (404 if unknown)
- sites.toml write reuses remove_site pattern (read toml→modify→toml::to_string_pretty→fs::write→sync in-memory SitesFile)
- Purge All Data stays disabled; label changed to "Coming soon"

---

### Task 1: Backend — fix set_default

**Files:**
- Modify: `crates/oxibuilder-console/src/router.rs` (set_default handler + route)
- Modify: `crates/oxibuilder-console/src/sites_runtime.rs` (+SiteRegistry::set_default)

- [ ] **Add `SiteRegistry::set_default`**

```rust
// sites_runtime.rs
pub async fn set_default(&self, slug: &str) -> anyhow::Result<()> {
    // 1. Validate slug is loaded
    if !self.sites.read().await.contains_key(slug) {
        anyhow::bail!("unknown site: {slug}");
    }
    // 2. Read/write sites.toml (mirror remove_site lines 178-201)
    let sites_path = directories::ProjectDirs::from("dev", "oxibuilder", "oxibuilder")
        .map(|p| p.config_dir().join("sites.toml"))
        .ok_or_else(|| anyhow::anyhow!("no config dir"))?;
    let mut sf = if sites_path.exists() {
        std::fs::read_to_string(&sites_path)
            .ok()
            .and_then(|raw| toml::from_str::<SitesFile>(&raw).ok())
            .unwrap_or_default()
    } else {
        SitesFile::default()
    };
    sf.set_default(slug.to_string());
    if let Some(parent) = sites_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let raw = toml::to_string_pretty(&sf)?;
    std::fs::write(&sites_path, raw)?;
    *self.sites_file.write().await = sf;
    Ok(())
}
```

- [ ] **Fix set_default handler**

```rust
// router.rs — replace lines 94-96
#[derive(Deserialize)]
struct SetDefaultInput { default_site: String }

async fn set_default(
    State(registry): State<Arc<SiteRegistry>>,
    Json(input): Json<SetDefaultInput>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    registry.set_default(&input.default_site).await
        .map_err(|e| (StatusCode::NOT_FOUND, e.to_string()))?;
    Ok(Json(serde_json::json!({"data": {"default_site": input.default_site}})))
}
```

- [ ] `cargo check -p oxibuilder-console`

---

### Task 2: Frontend — wire "Set as default" UI

**Files:**
- Modify: `web/src/admin/sites/SitesPage.tsx` (add "Set as default" action per row)
- Modify: `web/src/admin/settings/SettingsPage.tsx` (relabel Purge button, add default site selector)

- [ ] **SitesPage**: per-row action button "Set as default" → calls `setDefaultSite(slug)` → invalidates ["sites"]
- [ ] **SettingsPage**: Purge button label → "Purge All Data (Coming soon)", stays disabled. Add "Default Site" dropdown populated from listSites, default from getDefaultSite, save calls setDefaultSite.

- [ ] `cd web && npx tsc --noEmit`

---

### Task 3: Smoke test

- [ ] `cargo check && cd web && npx tsc --noEmit`
- [ ] Manual: set site as default → verify sites.toml changed → switch to another site → verify default_slug() reflects change → unknown slug → 404
