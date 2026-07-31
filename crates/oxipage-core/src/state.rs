use crate::builder::BuildExt;
use crate::config::Config;
use crate::extension::WasmLoader;
use crate::registry::ExtensionRegistry;
use sqlx::SqlitePool;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Per-site DB pool injected into request extensions by middleware.
///
/// Extensions extract it with `Extension(pool): Extension<SiteScopedDb>` and
/// use `pool.db` instead of the old `state.db` pattern. Defined in core so
/// all extension crates can reference it without depending on oxipage-console.
#[derive(Clone)]
pub struct SiteScopedDb {
    pub db: SqlitePool,
    /// Live-reloadable site settings (site languages etc.) for per-site
    /// extension handlers that validate against configuration.
    pub settings: std::sync::Arc<tokio::sync::RwLock<crate::site_paths::MutableSiteSettings>>,
}

/// 사이트명/URL 오버라이드 — setup 마법사가 oxipage.toml을 갱신한 후
/// 재시작 없이 런타임에 반영하기 위한 필드 (doc/13 §13.5.3).
/// lobby manifest 등 사이트명 표시부는 override → config.site 순으로 읽음.
#[derive(Debug, Clone)]
pub struct SiteOverride {
    pub name: String,
    pub base_url: String,
}

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub config: Arc<Config>,
    pub registry: Arc<ExtensionRegistry>,
    /// WASM 런타임 로더. `--features wasm` 서버 빌드에서만 Some.
    /// None 이면 install 엔드포인트가 파일만 쓰고 "restart to activate" 반환.
    pub wasm_loader: Option<Arc<dyn WasmLoader>>,
    /// Setup 마법사가 설정한 사이트명/URL 오버라이드 (doc/13).
    /// None = config.site 사용 (기본).
    pub site_override: Arc<RwLock<Option<SiteOverride>>>,
    /// BuildExt 인스턴스 (v2 SSG 빌드용).
    pub builders: Arc<Vec<Box<dyn BuildExt>>>,
}

impl AppState {
    /// site_override가 있으면 그 값을, 없으면 config.site.name 반환.
    pub async fn effective_site_name(&self) -> String {
        self.site_override
            .read()
            .await
            .as_ref()
            .map(|s| s.name.clone())
            .unwrap_or_else(|| self.config.site.name.clone())
    }

    /// site_override가 있으면 그 값을, 없으면 config.site.base_url 반환.
    pub async fn effective_base_url(&self) -> String {
        self.site_override
            .read()
            .await
            .as_ref()
            .map(|s| s.base_url.clone())
            .unwrap_or_else(|| self.config.site.base_url.clone())
    }
}
