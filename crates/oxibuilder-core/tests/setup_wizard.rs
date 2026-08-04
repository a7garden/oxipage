//! Setup wizard Extension hook integration tests.
//!
//! `Extension` trait의 `setup_wizard` / `seed_sample_data` hook이 setup API와 제대로 통합되는지 검증.
//! 코어가 도메인 필드를 모른다 — 확장이 자기 데이터를 제공한다.

use async_trait::async_trait;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use oxibuilder_core::config::Config;
use oxibuilder_core::extension::{
    Extension, ExtensionWizard, Lang, LobbyCard, Migration, SetupField, SetupFieldKind,
    SetupSaveHandler, SetupStep, StepOutcome, VisibilityRule,
};
use oxibuilder_core::registry::ExtensionRegistry;
use oxibuilder_core::state::AppState;
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
    ) -> anyhow::Result<StepOutcome> {
        Ok(StepOutcome::default())
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
    fn routes(&self) -> axum::Router {
        axum::Router::new()
    }

    async fn lobby_summary(&self, _ctx: &AppState) -> Option<LobbyCard> {
        None
    }
    fn setup_wizard(&self) -> Option<ExtensionWizard> {
        Some(ExtensionWizard {
            steps: vec![SetupStep {
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
                visible_when: None,
            }],
        })
    }
    async fn seed_sample_data(&self, _ctx: &AppState) -> anyhow::Result<()> {
        Ok(())
    }
}

struct NoopSave;
#[async_trait]
impl SetupSaveHandler for NoopSave {
    async fn save(
        &self,
        _ctx: &AppState,
        _form: &serde_json::Map<String, serde_json::Value>,
    ) -> anyhow::Result<StepOutcome> {
        Ok(StepOutcome::default())
    }
}

struct MultiStepExt;
#[async_trait]
impl Extension for MultiStepExt {
    fn id(&self) -> &'static str {
        "multi"
    }
    fn display_name(&self, _: Lang) -> String {
        "Multi".into()
    }
    fn migrations(&self) -> Vec<Migration> {
        vec![]
    }
    fn routes(&self) -> axum::Router {
        axum::Router::new()
    }

    async fn lobby_summary(&self, _ctx: &AppState) -> Option<LobbyCard> {
        None
    }
    fn setup_wizard(&self) -> Option<ExtensionWizard> {
        Some(ExtensionWizard {
            steps: vec![
                SetupStep {
                    id: "multi_a",
                    title_ko: "A",
                    title_en: "A",
                    description_ko: "",
                    description_en: "",
                    fields: vec![],
                    save_handler: Arc::new(NoopSave),
                    prefill: std::collections::BTreeMap::new(),
                    visible_when: None,
                },
                SetupStep {
                    id: "multi_b",
                    title_ko: "B",
                    title_en: "B",
                    description_ko: "",
                    description_en: "",
                    fields: vec![],
                    save_handler: Arc::new(NoopSave),
                    prefill: std::collections::BTreeMap::new(),
                    visible_when: None,
                },
            ],
        })
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
    fn routes(&self) -> axum::Router {
        axum::Router::new()
    }

    async fn lobby_summary(&self, _ctx: &AppState) -> Option<LobbyCard> {
        None
    }
    fn setup_wizard(&self) -> Option<ExtensionWizard> {
        Some(ExtensionWizard {
            steps: vec![SetupStep {
                id: "key_only_step",
                title_ko: "알파 키",
                title_en: "Alpha key",
                description_ko: "테스트용 키 step",
                description_en: "test key step",
                fields: vec![SetupField {
                    name: "alpha",
                    label_ko: "알파",
                    label_en: "Alpha",
                    kind: SetupFieldKind::Secret,
                    required: false,
                    placeholder_ko: None,
                    placeholder_en: None,
                }],
                save_handler: Arc::new(NoopSave),
                prefill: std::collections::BTreeMap::new(),
                visible_when: None,
            }],
        })
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
    let pool = oxibuilder_core::db::connect_memory().await.unwrap();
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
    with_loopback(oxibuilder_core::http::build_app(state))
}
async fn build_app(extensions: Vec<Arc<dyn Extension>>) -> axum::Router {
    let pool = oxibuilder_core::db::connect_memory().await.unwrap();
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
    with_loopback(oxibuilder_core::http::build_app(state))
}

#[tokio::test]
async fn unknown_step_returns_404() {
    let app = build_app(vec![Arc::new(DemoExt { step_id: "demo" })]).await;
    let res = app
        .oneshot(
            Request::post("/api/console/setup/extension-step/demo/missing")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn status_returns_multiple_steps_per_wizard() {
    let app = build_app(vec![Arc::new(MultiStepExt)]).await;
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
    let wizards = json["data"]["extension_wizards"].as_array().unwrap();
    assert_eq!(wizards.len(), 1);
    assert_eq!(wizards[0]["extension_id"], "multi");
    let steps = wizards[0]["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 2);
    assert_eq!(steps[0]["id"], "multi_a");
    assert_eq!(steps[1]["id"], "multi_b");
}

#[tokio::test]
async fn setup_status_includes_extension_wizards() {
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
    let wizards = json["data"]["extension_wizards"].as_array().unwrap();
    assert_eq!(wizards.len(), 1);
    assert_eq!(wizards[0]["extension_id"], "demo");
    let steps = wizards[0]["steps"].as_array().unwrap();
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

/// 미활성 확장은 extension_wizards에서 제외되어야 한다.
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
    let wizards = json["data"]["extension_wizards"].as_array().unwrap();
    let wizard_ids: Vec<&str> = wizards
        .iter()
        .map(|w| w["extension_id"].as_str().unwrap())
        .collect();
    assert!(
        !wizard_ids.contains(&"demo"),
        "disabled extension's wizard must be excluded, got: {wizard_ids:?}"
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
            Request::post("/api/console/setup/extension-step/demo/demo")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    // setup 단계에서 disabled 확장은 step 자체가 노출 안 되고 직접 POST도 거부됨.
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

struct OutcomeSave;
#[async_trait]
impl SetupSaveHandler for OutcomeSave {
    async fn save(
        &self,
        _ctx: &AppState,
        form: &serde_json::Map<String, serde_json::Value>,
    ) -> anyhow::Result<StepOutcome> {
        Ok(StepOutcome {
            values: form.clone(),
        })
    }
}

struct OutcomeExt;
#[async_trait]
impl Extension for OutcomeExt {
    fn id(&self) -> &'static str {
        "outcome"
    }
    fn display_name(&self, _: Lang) -> String {
        "Outcome".into()
    }
    fn migrations(&self) -> Vec<Migration> {
        vec![]
    }
    fn routes(&self) -> axum::Router {
        axum::Router::new()
    }

    async fn lobby_summary(&self, _ctx: &AppState) -> Option<LobbyCard> {
        None
    }
    fn setup_wizard(&self) -> Option<ExtensionWizard> {
        Some(ExtensionWizard {
            steps: vec![SetupStep {
                id: "step_one",
                title_ko: "1",
                title_en: "1",
                description_ko: "",
                description_en: "",
                fields: vec![],
                save_handler: Arc::new(OutcomeSave),
                prefill: std::collections::BTreeMap::new(),
                visible_when: None,
            }],
        })
    }
}

struct ConditionalExt;
#[async_trait]
impl Extension for ConditionalExt {
    fn id(&self) -> &'static str {
        "cond"
    }
    fn display_name(&self, _: Lang) -> String {
        "Cond".into()
    }
    fn migrations(&self) -> Vec<Migration> {
        vec![]
    }
    fn routes(&self) -> axum::Router {
        axum::Router::new()
    }

    async fn lobby_summary(&self, _ctx: &AppState) -> Option<LobbyCard> {
        None
    }
    fn setup_wizard(&self) -> Option<ExtensionWizard> {
        Some(ExtensionWizard {
            steps: vec![
                SetupStep {
                    id: "cond_a",
                    title_ko: "A",
                    title_en: "A",
                    description_ko: "",
                    description_en: "",
                    fields: vec![],
                    save_handler: Arc::new(NoopSave),
                    prefill: std::collections::BTreeMap::new(),
                    visible_when: None,
                },
                SetupStep {
                    id: "cond_b",
                    title_ko: "B",
                    title_en: "B",
                    description_ko: "",
                    description_en: "",
                    fields: vec![],
                    save_handler: Arc::new(NoopSave),
                    prefill: std::collections::BTreeMap::new(),
                    visible_when: Some(VisibilityRule::FieldNotEmpty {
                        step_id: "cond_a",
                        field: "x",
                    }),
                },
            ],
        })
    }
}

#[tokio::test]
async fn extension_step_returns_outcome() {
    let app = build_app(vec![Arc::new(OutcomeExt)]).await;
    let res = app
        .oneshot(
            Request::post("/api/console/setup/extension-step/outcome/step_one")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"name":"hi"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["data"]["values"]["name"], "hi");
}

#[tokio::test]
async fn status_serializes_visible_when() {
    let app = build_app(vec![Arc::new(ConditionalExt)]).await;
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
    let rule = &json["data"]["extension_wizards"][0]["steps"][1]["visible_when"];
    assert_eq!(rule["kind"], "field_not_empty");
    assert_eq!(rule["step_id"], "cond_a");
    assert_eq!(rule["field"], "x");
}
