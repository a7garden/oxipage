//! 첫 부팅 UX 설정 마법사 API (doc/13).
//!
//! 모든 핸들러는 `setup_gate` 미들웨어가 보호:
//! - loopback(127.0.0.1/::1) 외 403
//! - setup 완료 후 410 Gone

use crate::error::ApiError;
use crate::extension::DataEnvelope;
use crate::state::{AppState, SiteOverride};
use axum::extract::{ConnectInfo, Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::{Json, Router};
use axum::routing::{get, post};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

// ─── Constants ───

const SAMPLE_POST_TITLE: &str = "환영합니다";
const SAMPLE_POST_SLUG: &str = "환영합니다";
const SAMPLE_POST_BODY: &str = r#"# 환영합니다!

Oxipage 설치가 완료되었습니다.

이 글은 설정 마법사가 생성한 샘플 글입니다.
삭제하거나 수정해도 됩니다.

## 다음 단계

- **CLI**로 글 쓰기: `oxipage blog new "제목" --file draft.md`
- **관리 콘솔**에서 콘텐츠 관리: 헤더의 설정 버튼
- **프로젝트** 추가: `oxipage project add --title-ko "..." --title-en "..."`

즐거운 블로그 생활 되세요!
"#;

const THEMES: &[ThemeEntry] = &[
    ThemeEntry { id: "paper", name_ko: "종이", name_en: "Paper", mode: "light",
        description_ko: "따뜻한 종이 배경", description_en: "Warm paper background",
        preview_colors: ["#fafaf5", "#f5f2ed", "#2d2934", "#2d7a5c"] },
    ThemeEntry { id: "midnight", name_ko: "한밤", name_en: "Midnight", mode: "dark",
        description_ko: "깊은 밤하늘", description_en: "Deep night sky",
        preview_colors: ["#1a1a2e", "#16213e", "#e0e0e0", "#4fc3f7"] },
    ThemeEntry { id: "sepia", name_ko: "세피아", name_en: "Sepia", mode: "light",
        description_ko: "오래된 책장", description_en: "Old bookshelf",
        preview_colors: ["#f5f0e8", "#ede0d4", "#3d3529", "#b8860b"] },
    ThemeEntry { id: "forest", name_ko: "숲", name_en: "Forest", mode: "dark",
        description_ko: "이끼 낀 숲", description_en: "Mossy forest",
        preview_colors: ["#1b2b1b", "#243624", "#e0e8e0", "#2ecc71"] },
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
pub struct ProfileInput {
    pub display_name: Option<String>,
    pub tagline_ko: Option<String>,
    pub tagline_en: Option<String>,
    pub github_username: Option<String>,
    pub bio_ko: Option<String>,
    pub bio_en: Option<String>,
}

#[derive(Deserialize)]
pub struct ThemeInput {
    pub theme_id: String,
    pub lobby_mode: Option<String>,
}

#[derive(Deserialize)]
pub struct ContentInput {
    #[serde(default = "default_true")]
    pub sample_post: bool,
    pub tmdb_key: Option<String>,
    pub aladin_key: Option<String>,
}

fn default_true() -> bool { true }

#[derive(Serialize)]
pub struct CompleteResult {
    pub ok: bool,
    pub message: String,
}

#[derive(Serialize)]
pub struct StatusResult {
    pub setup_mode: bool,
    pub completed_steps: Vec<String>,
    pub available_extensions: Vec<ExtInfo>,
    pub available_themes: Vec<ThemeEntry>,
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
async fn get_completed_steps(state: &AppState) -> Result<Vec<String>, ApiError> {
    let mut steps = Vec::new();

    // site: site_name이 설정되었는가
    if let Ok((Some(_),)) = sqlx::query_as::<_, (Option<String>,)>(
        "SELECT site_name FROM setup_state WHERE id = 1",
    )
    .fetch_one(&state.db)
    .await
    {
        steps.push("site".into());
    }

    // admin: auth 폐지 — 비밀번호 단계 제거, 항상 완료로 처리
    steps.push("admin".into());

    // extensions: enabled가 1개 이상인가
    match sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM extension_state WHERE enabled = 1",
    )
    .fetch_one(&state.db)
    .await
    {
        Ok((count,)) if count > 0 => steps.push("extensions".into()),
        _ => {}
    }

    // profile: tagline이나 github가 설정되었는가
    match sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM profile WHERE id = 1 AND (tagline_ko IS NOT NULL OR github_username IS NOT NULL)",
    )
    .fetch_one(&state.db)
    .await
    {
        Ok((count,)) if count > 0 => steps.push("profile".into()),
        _ => {}
    }

    // content: 샘플 글이 생성되었거나, API 키가 설정되었는가
    match sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM blog_post WHERE slug = ?1",
    )
    .bind(SAMPLE_POST_SLUG)
    .fetch_one(&state.db)
    .await
    {
        Ok((count,)) if count > 0 => {
            steps.push("content".into());
        }
        _ => {}
    }

    // theme은 항상 통과 (step 5는 필수)
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

/// extension_state.config JSON 업데이트
async fn set_extension_config(state: &AppState, ext_id: &str, key: &str, value: &str) -> Result<(), ApiError> {
    if value.is_empty() {
        return Ok(());
    }
    let config_json = serde_json::json!({ key: value }).to_string();
    sqlx::query("UPDATE extension_state SET config = ?1 WHERE extension_id = ?2")
        .bind(&config_json)
        .bind(ext_id)
        .execute(&state.db)
        .await
        .map_err(|e| ApiError::internal(anyhow::anyhow!(e)))?;
    Ok(())
}


// ─── Route builder ───

/// Setup 라우트를 api Router에 추가한다.
/// setup_gate 미들웨어는 http.rs에서 별도 적용.
pub fn setup_routes(api: Router<AppState>) -> Router<AppState> {
    api.route("/setup/status", get(setup_status_handler))
        .route("/setup/site", post(setup_site_handler))
        .route("/setup/admin", post(setup_admin_handler))
        .route("/setup/extensions", post(setup_extensions_handler))
        .route("/setup/profile", post(setup_profile_handler))
        .route("/setup/theme", post(setup_theme_handler))
        .route("/setup/content", post(setup_content_handler))
        .route("/setup/complete", post(setup_complete_handler))
}

// ─── Middleware ───

/// Check if setup wizard should run (no setup_completed_at in setup_state).
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
pub async fn setup_gate(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
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
pub async fn setup_status_handler(
    State(state): State<AppState>,
) -> Result<Json<DataEnvelope<StatusResult>>, ApiError> {
    let completed_steps = get_completed_steps(&state).await?;

    let extensions: Vec<ExtInfo> = state
        .registry
        .iter()
        .into_iter()
        .map(|ext| ExtInfo {
            id: ext.id().to_string(),
            display_name: ExtDisplayName {
                ko: ext.display_name(crate::extension::Lang::Ko),
                en: ext.display_name(crate::extension::Lang::En),
            },
        })
        .collect();

    Ok(Json(DataEnvelope {
        data: StatusResult {
            setup_mode: true,
            completed_steps,
            available_extensions: extensions,
            available_themes: THEMES.to_vec(),
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
    let base_url = input.base_url.unwrap_or_else(|| "http://127.0.0.1:8787".into());

    // Persist
    let _ = update_toml_site(&name, &base_url);

    sqlx::query("UPDATE setup_state SET site_name = ?1, base_url = ?2 WHERE id = 1")
        .bind(&name)
        .bind(&base_url)
        .execute(&state.db)
        .await
        .map_err(|e| ApiError::internal(anyhow::anyhow!(e)))?;

    sqlx::query(
        "UPDATE profile SET display_name = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = 1",
    )
    .bind(&name)
    .execute(&state.db)
    .await
    .map_err(|e| ApiError::internal(anyhow::anyhow!(e)))?;

    *state.site_override.write().await = Some(SiteOverride {
        name: name.clone(),
        base_url: base_url.clone(),
    });

    Ok(Json(DataEnvelope {
        data: SimpleOk { ok: true },
    }))
}

/// POST /api/console/setup/admin — auth 폐지로 no-op (항상 ok).
pub async fn setup_admin_handler() -> Json<DataEnvelope<SimpleOk>> {
    Json(DataEnvelope { data: SimpleOk { ok: true } })
}

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

/// POST /api/console/setup/profile
pub async fn setup_profile_handler(
    State(state): State<AppState>,
    Json(input): Json<ProfileInput>,
) -> Result<Json<DataEnvelope<SimpleOk>>, ApiError> {
    let dn = input.display_name.as_deref().unwrap_or("");
    sqlx::query(
        "UPDATE profile SET
            display_name = COALESCE(NULLIF(?1, ''), display_name),
            tagline_ko = ?2, tagline_en = ?3,
            github_username = ?4, bio_ko = ?5, bio_en = ?6,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = 1",
    )
    .bind(dn)
    .bind(&input.tagline_ko)
    .bind(&input.tagline_en)
    .bind(&input.github_username)
    .bind(&input.bio_ko)
    .bind(&input.bio_en)
    .execute(&state.db)
    .await
    .map_err(|e| ApiError::internal(anyhow::anyhow!(e)))?;

    Ok(Json(DataEnvelope {
        data: SimpleOk { ok: true },
    }))
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

/// POST /api/console/setup/content
pub async fn setup_content_handler(
    State(state): State<AppState>,
    Json(input): Json<ContentInput>,
) -> Result<Json<DataEnvelope<SimpleOk>>, ApiError> {
    if input.sample_post {
        let exists: Result<(i64,), _> = sqlx::query_as(
            "SELECT COUNT(*) FROM blog_post WHERE slug = ?1",
        )
        .bind(SAMPLE_POST_SLUG)
        .fetch_one(&state.db)
        .await;

        if let Ok((0,)) = exists {
            sqlx::query(
                "INSERT INTO blog_post (slug, title, body, lang, tags, published_at, created_at, updated_at)
                 VALUES (?1, ?2, ?3, 'ko', '[]',
                         strftime('%Y-%m-%dT%H:%M:%fZ','now'),
                         strftime('%Y-%m-%dT%H:%M:%fZ','now'),
                         strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
            )
            .bind(SAMPLE_POST_SLUG)
            .bind(SAMPLE_POST_TITLE)
            .bind(SAMPLE_POST_BODY)
            .execute(&state.db)
            .await
            .map_err(|e| ApiError::internal(anyhow::anyhow!(e)))?;
        }
    }

    if let Some(key) = input.tmdb_key.as_ref().filter(|k| !k.is_empty()) {
        set_extension_config(&state, "movies", "tmdb_key", key).await?;
    }
    if let Some(key) = input.aladin_key.as_ref().filter(|k| !k.is_empty()) {
        set_extension_config(&state, "books", "aladin_key", key).await?;
    }

    Ok(Json(DataEnvelope {
        data: SimpleOk { ok: true },
    }))
}

/// POST /api/console/setup/complete
pub async fn setup_complete_handler(
    State(state): State<AppState>,
) -> Result<Json<DataEnvelope<CompleteResult>>, ApiError> {
    sqlx::query(
        "UPDATE setup_state SET setup_completed_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = 1",
    )
    .execute(&state.db)
    .await
    .map_err(|e| ApiError::internal(anyhow::anyhow!(e)))?;

    Ok(Json(DataEnvelope {
        data: CompleteResult {
            ok: true,
            message: "설정이 완료되었습니다.".into(),
        },
    }))
}
