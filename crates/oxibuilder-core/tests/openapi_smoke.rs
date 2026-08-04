//! OpenAPI smoke test: registry의 확장이 자동으로 paths에 등장하는지 검증.
//! 9개 실제 확장에 의존하지 않고 더미 확장으로 핵심 메커니즘을 검증한다.

use async_trait::async_trait;
use oxibuilder_core::extension::{Extension, Lang, LobbyCard, Migration};
use oxibuilder_core::registry::ExtensionRegistry;
use oxibuilder_core::state::AppState;
use std::sync::Arc;

struct FakeExt(&'static str);
#[async_trait]
impl Extension for FakeExt {
    fn id(&self) -> &'static str {
        self.0
    }
    fn display_name(&self, lang: Lang) -> String {
        match lang {
            Lang::Ko => "가짜".to_string(),
            Lang::En => "Fake".to_string(),
        }
    }
    fn migrations(&self) -> Vec<Migration> {
        vec![]
    }
    fn table_names(&self) -> Vec<&'static str> {
        vec![]
    }
    fn routes(&self) -> axum::Router {
        axum::Router::new()
    }

    async fn lobby_summary(&self, _ctx: &AppState) -> Option<LobbyCard> {
        None
    }
}

#[tokio::test]
async fn registry_extensions_appear_in_openapi_paths() {
    let all: Vec<Arc<dyn Extension>> = vec![
        Arc::new(FakeExt("alpha")),
        Arc::new(FakeExt("beta")),
        Arc::new(FakeExt("gamma")),
    ];
    let registry = ExtensionRegistry::new(all);

    let spec = oxibuilder_core::openapi::openapi_spec("http://localhost:8787", &registry);
    let paths = spec["paths"].as_object().expect("paths is object");

    for id in ["alpha", "beta", "gamma"] {
        let base = format!("/api/console/{id}/");
        let item = format!("/api/console/{id}/{{slug}}");
        let publish = format!("/api/console/{id}/{{slug}}/publish");
        assert!(paths.contains_key(&base), "missing path: {base}");
        assert!(paths.contains_key(&item), "missing path: {item}");
        assert!(paths.contains_key(&publish), "missing path: {publish}");
    }

    // 코어 시스템 경로도 함께 있어야 함
    assert!(paths.contains_key("/api/console/setup/status"));
    assert!(paths.contains_key("/api/console/setup/extension-step/{ext_id}/{step_id}"));
    assert!(paths.contains_key("/api/console/lobby/manifest"));
    assert!(paths.contains_key("/healthz"));
}

#[tokio::test]
async fn empty_registry_omits_extension_paths() {
    let registry = ExtensionRegistry::new(vec![]);

    let spec = oxibuilder_core::openapi::openapi_spec("", &registry);
    let paths = spec["paths"].as_object().expect("paths is object");

    // 확장 경로는 없어야 함
    for id in ["alpha", "beta", "profile", "blog"] {
        let base = format!("/api/console/{id}/");
        assert!(!paths.contains_key(&base), "unexpected path: {base}");
    }
    // 코어 경로는 유지
    assert!(paths.contains_key("/healthz"));
}
