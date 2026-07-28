use crate::config::Config;
use crate::extension::WasmLoader;
use crate::registry::ExtensionRegistry;
use sqlx::SqlitePool;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub config: Arc<Config>,
    pub admin_token: Option<Arc<str>>,
    pub registry: Arc<ExtensionRegistry>,
    /// WASM 런타임 로더. `--features wasm` 서버 빌드에서만 Some.
    /// None 이면 install 엔드포인트가 파일만 쓰고 "restart to activate" 반환.
    pub wasm_loader: Option<Arc<dyn WasmLoader>>,
}
