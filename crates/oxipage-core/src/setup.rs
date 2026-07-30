//! 첫 부팅 UX 설정 마법사 API (doc/13).
//!
//! 모든 핸들러는 `setup_gate` 미들웨어가 보호:
//! - loopback(127.0.0.1/::1) 외 403
//! - setup 완료 후 410 Gone

use crate::error::ApiError;
use crate::extension::{DataEnvelope, ExtensionStepInfo, StepOutcome};
use crate::state::{AppState, SiteOverride};
use axum::extract::{ConnectInfo, Path, Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

// 환영 글 (샘플 post) 데이터는 `oxipage-ext-blog`가 자기 `seed_sample_data()`에서
// 정의/관리한다. 코어는 블로그 도메인 데이터를 더 이상 모른다.

const THEMES: &[ThemeEntry] = &[
    ThemeEntry {
        id: "paper",
        name_ko: "종이",
        name_en: "Paper",
        mode: "light",
        description_ko: "따뜻한 종이 배경",
        description_en: "Warm paper background",
        preview_colors: ["#fafaf5", "#f5f2ed", "#2d2934", "#2d7a5c"],
    },
    ThemeEntry {
        id: "midnight",
        name_ko: "한밤",
        name_en: "Midnight",
        mode: "dark",
        description_ko: "깊은 밤하늘",
        description_en: "Deep night sky",
        preview_colors: ["#1a1a2e", "#16213e", "#e0e0e0", "#4fc3f7"],
    },
    ThemeEntry {
        id: "sepia",
        name_ko: "세피아",
        name_en: "Sepia",
        mode: "light",
        description_ko: "오래된 책장",
        description_en: "Old bookshelf",
        preview_colors: ["#f5f0e8", "#ede0d4", "#3d3529", "#b8860b"],
    },
    ThemeEntry {
        id: "forest",
        name_ko: "숲",
        name_en: "Forest",
        mode: "dark",
        description_ko: "이끼 낀 숲",
        description_en: "Mossy forest",
        preview_colors: ["#1b2b1b", "#243624", "#e0e8e0", "#2ecc71"],
    },
];

// ─── Input / Output types ───

#[derive(Deserialize)]
pub struct SiteInput {
    pub name: String,
    pub base_url: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct ExtensionsInput {
    pub enabled: Vec<String>,
}

#[derive(Deserialize)]
pub struct ThemeInput {
    pub theme_id: String,
    pub lobby_mode: Option<String>,
}

#[derive(Serialize)]
pub struct CompleteResult {
    pub ok: bool,
    pub message: String,
}

/// `GET /api/console/setup/status` 응답.
#[derive(Serialize)]
pub struct StatusResult {
    pub setup_mode: bool,
    pub completed_steps: Vec<String>,
    pub available_extensions: Vec<ExtInfo>,
    pub available_themes: Vec<ThemeEntry>,
    /// 활성 확장이 소유한 서브-위자드 (각각 0..N step).
    pub extension_wizards: Vec<ExtensionWizardInfo>,
}

#[derive(Serialize)]
pub struct ExtInfo {
    pub id: String,
    pub display_name: ExtDisplayName,
}

#[derive(Serialize)]
pub struct ExtDisplayName {
    pub ko: String,
    pub en: String,
}

/// 활성 확장의 서브-위자드 (status 응답용 직렬화 형태).
#[derive(Serialize)]
pub struct ExtensionWizardInfo {
    pub extension_id: String,
    pub display_name: ExtDisplayName,
    pub steps: Vec<ExtensionStepInfo>,
}

#[derive(Serialize, Clone)]
pub struct ThemeEntry {
    pub id: &'static str,
    pub name_ko: &'static str,
    pub name_en: &'static str,
    pub mode: &'static str,
    pub description_ko: &'static str,
    pub description_en: &'static str,
    pub preview_colors: [&'static str; 4],
}

#[derive(Serialize)]
pub struct SimpleOk {
    pub ok: bool,
}

// ─── Helpers ───

/// 완료된 step 목록 조회.
///
/// **확장 step은 동적이므로 core가 추적하지 않는다** — wizard UI가
/// `extension_wizards`로 자체 조립한다. 이 함수는 코어
/// 자체 step(site/extensions/theme)과 `admin` 호환 마커만 반환.
async fn get_completed_steps(state: &AppState) -> Result<Vec<String>, ApiError> {
    let mut steps = Vec::new();

    // site: site_name이 설정되었는가
    if let Ok((Some(_),)) =
        sqlx::query_as::<_, (Option<String>,)>("SELECT site_name FROM setup_state WHERE id = 1")
            .fetch_one(&state.db)
            .await
    {
        steps.push("site".into());
    }

    // admin: auth 폐지 — 비밀번호 단계 제거, 항상 완료로 처리 (하위 호환)
    steps.push("admin".into());

    // extensions: enabled가 1개 이상인가
    match sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM extension_state WHERE enabled = 1")
        .fetch_one(&state.db)
        .await
    {
        Ok((count,)) if count > 0 => steps.push("extensions".into()),
        _ => {}
    }

    // theme은 항상 통과 (step은 필수)
    steps.push("theme".into());

    Ok(steps)
}

/// TOML 파일 갱신 (site name)
fn update_toml_site(name: &str, base_url: &str) -> anyhow::Result<()> {
    let config_path = std::env::var("OXIPAGE_CONFIG")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("oxipage.toml"));

    if !config_path.exists() {
        return Ok(());
    }

    let content = std::fs::read_to_string(&config_path)?;
    let mut toml_val: toml::Value = content.parse::<toml::Value>()?;

    if let Some(table) = toml_val.get_mut("site").and_then(|s| s.as_table_mut()) {
        table.insert("name".into(), toml::Value::String(name.into()));
        table.insert("base_url".into(), toml::Value::String(base_url.into()));
    }

    let out = toml::to_string(&toml_val)?;
    std::fs::write(&config_path, out)?;
    Ok(())
}

// `set_extension_config`은 `crate::extension::persist_extension_config`로 이전.
// 트레이트 기본 impl이 직접 사용한다.

// ─── Route builder ───

/// Setup 라우트를 api Router에 추가한다.
/// setup_gate 미들웨어는 http.rs에서 별도 적용.
///
/// **확장 step과 외부 API 키 라우트는 registry 디스패치** — 코어는
/// 도메인 필드를 모른다. 확장이 자기 `SetupStep`으로
/// 동적으로 제공한다.
pub fn setup_routes(api: Router<AppState>) -> Router<AppState> {
    api.route("/setup/status", get(setup_status_handler))
        .route("/setup/site", post(setup_site_handler))
        .route("/setup/extensions", post(setup_extensions_handler))
        .route(
            "/setup/extension-step/{ext_id}/{step_id}",
            post(setup_extension_step_handler),
        )
        .route("/setup/theme", post(setup_theme_handler))
        .route("/setup/complete", post(setup_complete_handler))
}
// ─── Middleware ───

/// Check if setup wizard should run (no setup_completed_at in setup_state).
///
/// **Deprecated in favor of `oxipage_console::console_state::ConsoleState::is_setup_needed`.**
/// This function reads from **console.db** but is called with the per-site DB pool.
/// It happens to work because of the same table name, but the console's new
/// `ConsoleState` is the canonical source.
#[deprecated(note = "Use oxipage_console::console_state::ConsoleState::is_setup_needed")]
pub async fn is_setup_needed(db: &sqlx::SqlitePool) -> bool {
    matches!(
        sqlx::query_as::<_, (Option<String>,)>(
            "SELECT setup_completed_at FROM setup_state WHERE id = 1",
        )
        .fetch_one(db)
        .await,
        Ok((None,))
    )
}

/// Setup 게이트 미들웨어.
/// `/setup/*` 경로에만 적용: loopback 외 403, 완료 후 410.
pub async fn setup_gate(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let path = request.uri().path();
    let is_setup = path.contains("/setup/");

    if is_setup {
        // 1. Loopback check
        let is_loopback = request
            .extensions()
            .get::<ConnectInfo<SocketAddr>>()
            .is_some_and(|ci| ci.0.ip().is_loopback());
        if !is_loopback {
            return ApiError::new(
                StatusCode::FORBIDDEN,
                "setup_loopback_only",
                "setup API is only available from localhost",
            )
            .into_response();
        }

        // 2. Setup completed check
        match sqlx::query_as::<_, (Option<String>,)>(
            "SELECT setup_completed_at FROM setup_state WHERE id = 1",
        )
        .fetch_one(&state.db)
        .await
        {
            Ok((Some(_),)) => {
                return ApiError::new(
                    StatusCode::GONE,
                    "setup_completed",
                    "setup has already been completed",
                )
                .into_response();
            }
            Err(e) => {
                return ApiError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal",
                    &format!("setup state check failed: {e}"),
                )
                .into_response();
            }
            _ => {}
        }
    }

    next.run(request).await
}

// ─── Handlers ───

/// GET /api/console/setup/status
///
/// **동적 step 조립:** 활성 확장만 `extension_steps`에 포함, 외부 API 키는
/// 활성 확장이 노출한 모든 키를 평탄화해 id 중복은 마지막 우선으로 반환.
pub async fn setup_status_handler(
    State(state): State<AppState>,
) -> Result<Json<DataEnvelope<StatusResult>>, ApiError> {
    let completed_steps = get_completed_steps(&state).await?;

    let snapshot = state.registry.iter();
    let mut available_extensions: Vec<ExtInfo> = Vec::with_capacity(snapshot.len());
    let mut extension_wizards: Vec<ExtensionWizardInfo> = Vec::new();

    for ext in snapshot {
        available_extensions.push(ExtInfo {
            id: ext.id().to_string(),
            display_name: ExtDisplayName {
                ko: ext.display_name(crate::extension::Lang::Ko),
                en: ext.display_name(crate::extension::Lang::En),
            },
        });

        if !state.registry.is_active(ext.id()).await {
            continue;
        }

        if let Some(wizard) = ext.setup_wizard() {
            let steps: Vec<ExtensionStepInfo> = wizard
                .steps
                .iter()
                .map(ExtensionStepInfo::from_step)
                .collect();
            extension_wizards.push(ExtensionWizardInfo {
                extension_id: ext.id().to_string(),
                display_name: ExtDisplayName {
                    ko: ext.display_name(crate::extension::Lang::Ko),
                    en: ext.display_name(crate::extension::Lang::En),
                },
                steps,
            });
        }
    }

    Ok(Json(DataEnvelope {
        data: StatusResult {
            setup_mode: true,
            completed_steps,
            available_extensions,
            available_themes: THEMES.to_vec(),
            extension_wizards,
        },
    }))
}

/// POST /api/console/setup/site
pub async fn setup_site_handler(
    State(state): State<AppState>,
    Json(input): Json<SiteInput>,
) -> Result<Json<DataEnvelope<SimpleOk>>, ApiError> {
    let name = input.name.trim().to_string();
    if name.is_empty() || name.len() > 50 {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_site_name",
            "site name must be 1-50 characters",
        ));
    }
    let base_url = input
        .base_url
        .unwrap_or_else(|| "http://127.0.0.1:8787".into());

    // Persist
    let _ = update_toml_site(&name, &base_url);

    sqlx::query("UPDATE setup_state SET site_name = ?1, base_url = ?2 WHERE id = 1")
        .bind(&name)
        .bind(&base_url)
        .execute(&state.db)
        .await
        .map_err(|e| ApiError::internal(anyhow::anyhow!(e)))?;
    // `name`을 profile 표로 흘리는 것은 profile 확장의 `setup_wizard`가 처리한다.
    // 코어는 site_identity (setup_state + site_override)만 다룬다.

    *state.site_override.write().await = Some(SiteOverride {
        name: name.clone(),
        base_url: base_url.clone(),
    });

    Ok(Json(DataEnvelope {
        data: SimpleOk { ok: true },
    }))
}

// setup_admin 핸들러는 auth 폐지로 no-op이라 제거됨 (setup_routes에서도 제외).

/// POST /api/console/setup/extensions
pub async fn setup_extensions_handler(
    State(state): State<AppState>,
    Json(input): Json<ExtensionsInput>,
) -> Result<Json<DataEnvelope<ExtensionsInput>>, ApiError> {
    for id in &input.enabled {
        if state.registry.find(id).is_none() {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "unknown_extension",
                &format!("unknown extension: {id}"),
            ));
        }
    }

    // Disable all, then enable selected
    sqlx::query("UPDATE extension_state SET enabled = 0")
        .execute(&state.db)
        .await
        .map_err(|e| ApiError::internal(anyhow::anyhow!(e)))?;

    for id in &input.enabled {
        let count = sqlx::query("UPDATE extension_state SET enabled = 1 WHERE extension_id = ?1")
            .bind(id)
            .execute(&state.db)
            .await
            .map_err(|e| ApiError::internal(anyhow::anyhow!(e)))?;
        if count.rows_affected() == 0 {
            sqlx::query(
                "INSERT INTO extension_state (extension_id, enabled, purged) VALUES (?1, 1, 0)",
            )
            .bind(id)
            .execute(&state.db)
            .await
            .map_err(|e| ApiError::internal(anyhow::anyhow!(e)))?;
        }
    }

    // Update runtime registry cache
    for ext in state.registry.iter().into_iter() {
        let enabled = input.enabled.iter().any(|id| id == ext.id());
        let _ = state
            .registry
            .set_enabled(&state.db, ext.id(), enabled)
            .await;
    }

    Ok(Json(DataEnvelope {
        data: ExtensionsInput {
            enabled: input.enabled.clone(),
        },
    }))
}

/// **registry 디스패치:** `{id}`에 해당하는 `SetupStep`을 **활성 확장**에서 찾는다.
/// 비활성 확장의 step은 status에서 노출되지 않으므로 직접 POST도 거부한다 — 일관성.
/// 코어는 form의 필드 키/타입을 모른다 — 확장이 자기 트레이트 안에서 처리.
pub async fn setup_extension_step_handler(
    State(state): State<AppState>,
    Path((ext_id, step_id)): Path<(String, String)>,
    Json(form): Json<serde_json::Map<String, serde_json::Value>>,
) -> Result<Json<DataEnvelope<StepOutcome>>, ApiError> {
    for ext in state.registry.iter() {
        if ext.id() != ext_id {
            continue;
        }
        if !state.registry.is_active(ext.id()).await {
            continue;
        }
        if let Some(wizard) = ext.setup_wizard()
            && let Some(step) = wizard.steps.iter().find(|s| s.id == step_id)
        {
            let outcome = step
                .save_handler
                .save(&state, &form)
                .await
                .map_err(ApiError::internal)?;
            return Ok(Json(DataEnvelope { data: outcome }));
        }
    }
    Err(ApiError::new(
        StatusCode::NOT_FOUND,
        "unknown_step",
        &format!("no such extension setup step: {ext_id}/{step_id}"),
    ))
}

/// POST /api/console/setup/theme
pub async fn setup_theme_handler(
    State(state): State<AppState>,
    Json(input): Json<ThemeInput>,
) -> Result<Json<DataEnvelope<SimpleOk>>, ApiError> {
    if !THEMES.iter().any(|t| t.id == input.theme_id) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_theme",
            &format!("unknown theme: {}", input.theme_id),
        ));
    }

    sqlx::query("UPDATE theme_config SET theme_id = ?1, updated_at = datetime('now') WHERE id = 1")
        .bind(&input.theme_id)
        .execute(&state.db)
        .await
        .map_err(|e| ApiError::internal(anyhow::anyhow!(e)))?;

    if let Some(ref mode) = input.lobby_mode {
        if !matches!(mode.as_str(), "canvas" | "grid" | "list") {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_lobby_mode",
                "lobby mode must be canvas|grid|list",
            ));
        }
        sqlx::query("UPDATE lobby_config SET display_mode = ?1")
            .bind(mode)
            .execute(&state.db)
            .await
            .map_err(|e| ApiError::internal(anyhow::anyhow!(e)))?;
    }

    Ok(Json(DataEnvelope {
        data: SimpleOk { ok: true },
    }))
}

/// POST /api/console/setup/complete
///
/// **활성 확장의 `seed_sample_data()`를 호출**해 자기 도메인의 초기 데이터를 시드한다
/// (예: blog 확장이 환영 글 INSERT). 코어는 더 이상 어떤 도메인 데이터도 직접 쓰지 않는다.
/// 실패는 best-effort — 한 확장의 실패가 setup 완료를 막지 않는다.
pub async fn setup_complete_handler(
    State(state): State<AppState>,
) -> Result<Json<DataEnvelope<CompleteResult>>, ApiError> {
    sqlx::query(
        "UPDATE setup_state SET setup_completed_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = 1",
    )
    .execute(&state.db)
    .await
    .map_err(|e| ApiError::internal(anyhow::anyhow!(e)))?;

    // 시드는 setup_completed_at 마킹 후에도 진행 — 마킹은 admin 권한을 영구 적용하는 행위이고
    // 시드는 데이터 생성 행위라 의미가 다르다. 실패해도 200 반환.
    for ext in state.registry.iter() {
        if !state.registry.is_active(ext.id()).await {
            continue;
        }
        if let Err(e) = ext.seed_sample_data(&state).await {
            tracing::warn!(extension = ext.id(), error = %e, "seed_sample_data failed");
        }
    }

    Ok(Json(DataEnvelope {
        data: CompleteResult {
            ok: true,
            message: "설정이 완료되었습니다.".into(),
        },
    }))
}
