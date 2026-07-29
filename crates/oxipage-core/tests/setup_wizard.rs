//! Setup wizard Extension hook integration tests.
//!
//! `Extension` trait의 `setup_wizard_step` / `external_api_keys` /
//! `seed_sample_data` hook이 setup API와 제대로 통합되는지 검증.
//! 코어가 도메인 필드를 모른다 — 확장이 자기 데이터를 제공한다.

use async_trait::async_trait;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use oxipage_core::config::Config;
use oxipage_core::extension::{
    Extension, ExternalApiKey, ExternalKeyScope, Lang, LobbyCard, Migration, SetupField,
    SetupFieldKind, SetupSaveHandler, SetupStep,
};
use oxipage_core::registry::ExtensionRegistry;
use oxipage_core::state::AppState;
use std::sync::Arc;
use tower::ServiceExt;

// ─── 테스트용 확장 ────────────────────────────────────────

struct DemoExt {
    step_id: &'static str,
}

struct DemoSaveHandler;

#[async_trait]
impl SetupSaveHandler for DemoSaveHandler {
    async fn save(
        &self,
        _ctx: &AppState,
        _form: &serde_json::Map<String, serde_json::Value>,
    ) -> anyhow::Result<()> {
        Ok(())
    }
}

#[async_trait]
impl Extension for DemoExt {
    fn id(&self) -> &'static str {
        "demo"
    }
    fn display_name(&self, lang: Lang) -> String {
        match lang {
            Lang::Ko => "데모".to_string(),
            Lang::En => "Demo".to_string(),
        }
    }
    fn migrations(&self) -> Vec<Migration> {
        vec![Migration {
            version: 1,
            name: "init",
            sql: "CREATE TABLE IF NOT EXISTS demo_t (id INTEGER PRIMARY KEY, name TEXT)",
        }]
    }
    fn table_names(&self) -> Vec<&'static str> {
        vec!["demo_t"]
    }
    fn routes(&self) -> axum::Router<AppState> {
        axum::Router::new()
    }
    async fn lobby_summary(&self, _ctx: &AppState) -> Option<LobbyCard> {
        None
    }
    fn setup_wizard_step(&self) -> Option<SetupStep> {
        Some(SetupStep {
            id: self.step_id,
            title_ko: "데모 step",
            title_en: "Demo step",
            description_ko: "데모 step 설명",
            description_en: "Demo step description",
            fields: vec![SetupField {
                name: "name",
                label_ko: "이름",
                label_en: "Name",
                kind: SetupFieldKind::Text,
                required: true,
                placeholder_ko: None,
                placeholder_en: None,
            }],
            save_handler: Arc::new(DemoSaveHandler),
            prefill: std::collections::BTreeMap::new(),
        })
    }
    fn external_api_keys(&self) -> Vec<ExternalApiKey> {
        vec![ExternalApiKey {
            id: "demo_key",
            label_ko: "데모 키",
            label_en: "Demo key",
            env_var: "OXIPAGE_DEMO_KEY",
            required: false,
            scope: ExternalKeyScope::EnvOnly,
        }]
    }
    async fn seed_sample_data(&self, _ctx: &AppState) -> anyhow::Result<()> {
        Ok(())
    }
}

struct KeyOnlyExt;

#[async_trait]
impl Extension for KeyOnlyExt {
    fn id(&self) -> &'static str {
        "key_only"
    }
    fn display_name(&self, lang: Lang) -> String {
        match lang {
            Lang::Ko => "키온리".to_string(),
            Lang::En => "KeyOnly".to_string(),
        }
    }
    fn migrations(&self) -> Vec<Migration> {
        vec![]
    }
    fn table_names(&self) -> Vec<&'static str> {
        vec![]
    }
    fn routes(&self) -> axum::Router<AppState> {
        axum::Router::new()
    }
    async fn lobby_summary(&self, _ctx: &AppState) -> Option<LobbyCard> {
        None
    }
    fn external_api_keys(&self) -> Vec<ExternalApiKey> {
        vec![ExternalApiKey {
            id: "alpha",
            label_ko: "알파",
            label_en: "Alpha",
            env_var: "OXIPAGE_ALPHA",
            required: false,
            scope: ExternalKeyScope::EnvOnly,
        }]
    }
}
/// build_app 결과 router에 loopback `ConnectInfo<SocketAddr>`를 주입.
/// setup_gate 미들웨어가 loopback 검증에 의존하므로 테스트에서 필요.
async fn inject_loopback(
    mut req: Request<Body>,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let addr: std::net::SocketAddr = "127.0.0.1:12345".parse().unwrap();
    req.extensions_mut()
        .insert(axum::extract::ConnectInfo(addr));
    next.run(req).await
}

fn with_loopback(router: axum::Router) -> axum::Router {
    router.layer(axum::middleware::from_fn(inject_loopback))
}

async fn build_app_with_toml_enabled(
    extensions: Vec<Arc<dyn Extension>>,
    toml_enabled: &[String],
) -> axum::Router {
    let pool = oxipage_core::db::connect_memory().await.unwrap();
    let registry = Arc::new(ExtensionRegistry::new(extensions));
    registry.run_migrations(&pool, toml_enabled).await.unwrap();
    let state = AppState {
        db: pool,
        config: Arc::new(Config::default()),
        registry,
        wasm_loader: None,
        site_override: Arc::new(tokio::sync::RwLock::new(None)),
        builders: Arc::new(vec![]),
    };
    with_loopback(oxipage_core::http::build_app(state))
}
async fn build_app(extensions: Vec<Arc<dyn Extension>>) -> axum::Router {
    let pool = oxipage_core::db::connect_memory().await.unwrap();
    let registry = Arc::new(ExtensionRegistry::new(extensions));
    registry.run_migrations(&pool, &[]).await.unwrap();
    let state = AppState {
        db: pool,
        config: Arc::new(Config::default()),
        registry,
        wasm_loader: None,
        site_override: Arc::new(tokio::sync::RwLock::new(None)),
        builders: Arc::new(vec![]),
    };
    with_loopback(oxipage_core::http::build_app(state))
}

#[tokio::test]
async fn unknown_step_returns_404() {
    let app = build_app(vec![Arc::new(DemoExt { step_id: "demo" })]).await;
    let res = app
        .oneshot(
            Request::post("/api/console/setup/extension-step/missing")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn setup_status_includes_extension_steps() {
    let app = build_app(vec![Arc::new(DemoExt { step_id: "demo" })]).await;
    let res = app
        .oneshot(
            Request::get("/api/console/setup/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let steps = json["data"]["extension_steps"].as_array().unwrap();
    assert_eq!(steps.len(), 1);
    assert_eq!(steps[0]["id"], "demo");
    assert_eq!(steps[0]["title_ko"], "데모 step");
    assert_eq!(steps[0]["fields"][0]["name"], "name");
    // SetupFieldKind가 flatten되어야 클라이언트가 `field.type`로 직접 읽을 수 있다.
    // (bug 회귀: nested {kind:{type:...}} 직렬화 시 textarea가 input으로 렌더됨.)
    let field = &steps[0]["fields"][0];
    assert_eq!(
        field["type"], "text",
        "SetupFieldKind must serialize as top-level `type`"
    );
    assert!(
        field.get("kind").is_none(),
        "SetupFieldKind must be flattened, not nested under `kind`"
    );
}

#[tokio::test]
async fn setup_status_includes_external_api_keys() {
    let app = build_app(vec![
        Arc::new(DemoExt { step_id: "demo" }),
        Arc::new(KeyOnlyExt),
    ])
    .await;
    let res = app
        .oneshot(
            Request::get("/api/console/setup/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let keys = json["data"]["external_api_keys"].as_array().unwrap();
    let ids: Vec<&str> = keys.iter().map(|k| k["id"].as_str().unwrap()).collect();
    assert!(ids.contains(&"demo_key"));
    assert!(ids.contains(&"alpha"));
}

#[tokio::test]
async fn external_keys_handler_dispatches_to_extension() {
    let app = build_app(vec![Arc::new(DemoExt { step_id: "demo" })]).await;
    let res = app
        .oneshot(
            Request::post("/api/console/setup/external-keys")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"values":{"demo_key":"secret"}}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    // env var가 process env에 set되었는지 확인 (EnvOnly scope)
    assert_eq!(
        std::env::var("OXIPAGE_DEMO_KEY").ok().as_deref(),
        Some("secret"),
    );
}

/// 미활성 확장은 extension_steps/external_api_keys에서 제외되어야 한다.
/// (P2 §is_active gate 회귀 테스트 — 이 게이트를 제거하면 모든 테스트가 그대로 통과한다.)
#[tokio::test]
async fn disabled_extension_excluded_from_status() {
    // demo 비활성, key_only만 활성.
    let app = build_app_with_toml_enabled(
        vec![Arc::new(DemoExt { step_id: "demo" }), Arc::new(KeyOnlyExt)],
        &["key_only".to_string()],
    )
    .await;
    let res = app
        .oneshot(
            Request::get("/api/console/setup/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let steps = json["data"]["extension_steps"].as_array().unwrap();
    let step_ids: Vec<&str> = steps.iter().map(|s| s["id"].as_str().unwrap()).collect();
    assert!(
        !step_ids.contains(&"demo"),
        "disabled extension's step must be excluded, got: {step_ids:?}"
    );
    let keys = json["data"]["external_api_keys"].as_array().unwrap();
    let key_ids: Vec<&str> = keys.iter().map(|k| k["id"].as_str().unwrap()).collect();
    assert!(
        !key_ids.contains(&"demo_key"),
        "disabled extension's key must be excluded, got: {key_ids:?}"
    );
    assert!(
        key_ids.contains(&"alpha"),
        "active extension's key must remain"
    );
}

/// disable된 확장의 extension-step POST는 404로 거부되어야 한다.
#[tokio::test]
async fn disabled_extension_step_returns_404() {
    let app = build_app_with_toml_enabled(
        vec![Arc::new(DemoExt { step_id: "demo" }), Arc::new(KeyOnlyExt)],
        &["key_only".to_string()],
    )
    .await;
    let res = app
        .oneshot(
            Request::post("/api/console/setup/extension-step/demo")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    // setup 단계에서 disabled 확장은 step 자체가 노출 안 되고 직접 POST도 거부됨.
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

/// external_keys POST는 disabled 확장의 키를 침묵히 무시해야 한다.
/// (admin이 실수로 구 키를 보내도 안전.)
#[tokio::test]
async fn external_keys_ignores_disabled_extension_keys() {
    let app = build_app_with_toml_enabled(
        vec![Arc::new(DemoExt { step_id: "demo" }), Arc::new(KeyOnlyExt)],
        &["key_only".to_string()],
    )
    .await;
    let res = app
        .oneshot(
            Request::post("/api/console/setup/external-keys")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"values":{"demo_key":"should-be-ignored","alpha":"kept"}}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    // demo_key (disabled): env 안 바뀌어야 함.
    assert_ne!(
        std::env::var("OXIPAGE_DEMO_KEY").ok().as_deref(),
        Some("should-be-ignored")
    );
    // alpha (active): env에 저장됨.
    assert_eq!(std::env::var("OXIPAGE_ALPHA").ok().as_deref(), Some("kept"));
}
