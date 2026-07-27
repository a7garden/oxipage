//! OpenAPI 스펙 (doc/04 §4.5).
//!
//! **편차:** `utoipa` 자동 생성 대신 수동 `serde_json` 스펙. 의존성 절약 + 코어
//! 라우트를 즉시 문서화. 확장 라우트는 차후 `Extension::openapi_paths()` 훅으로
//! 병합(별도 작업). `/api/v1/docs/openapi.json` + `/api/v1/docs`(Swagger UI CDN).

use serde_json::{json, Value};

pub fn openapi_spec(base_url: &str) -> Value {
    let server = if base_url.is_empty() {
        "/".to_string()
    } else {
        format!("{}/", base_url.trim_end_matches('/'))
    };
    json!({
        "openapi": "3.0.3",
        "info": {
            "title": "Oxipage API",
            "version": "0.1.0",
            "description": "개인 창작 작업실 홈페이지 API (doc/04 §4.5). 공개 읽기는 인증 불필요, 쓰기는 Bearer 토큰(PAT 또는 OXIPAGE_ADMIN_TOKEN).",
            "license": { "name": "MIT" }
        },
        "servers": [{ "url": server }],
        "components": {
            "securitySchemes": {
                "bearerAuth": { "type": "http", "scheme": "bearer" }
            }
        },
        "paths": {
            "/api/v1/lobby/manifest": {
                "get": {
                    "tags": ["lobby"],
                    "summary": "로비 매니페스트 (site + extensions + per-extension lobby config)",
                    "security": [],
                    "responses": ok_data("Manifest")
                }
            },
            "/api/v1/lobby/config": {
                "get": {
                    "tags": ["lobby"],
                    "summary": "모든 확장의 로비 표시 설정",
                    "security": [],
                    "responses": ok_data("LobbyConfig[]")
                }
            },
            "/api/v1/lobby/config/{ext_id}": {
                "put": {
                    "tags": ["lobby"],
                    "summary": "확장별 로비 표시 모드/순서 갱신 (post:write)",
                    "security": [{ "bearerAuth": [] }],
                    "parameters": [{ "name": "ext_id", "in": "path", "required": true, "schema": { "type": "string" } }],
                    "responses": ok_data("LobbyConfig")
                }
            },
            "/api/v1/auth/tokens": {
                "get": {
                    "tags": ["auth"],
                    "summary": "PAT 목록 (admin 스코프)",
                    "security": [{ "bearerAuth": [] }],
                    "responses": ok_data("PatRow[]")
                },
                "post": {
                    "tags": ["auth"],
                    "summary": "새 PAT 발급 (admin 스코프). 평문은 1회만 반환.",
                    "security": [{ "bearerAuth": [] }],
                    "responses": ok_data("PatCreated")
                }
            },
            "/api/v1/auth/tokens/{id}": {
                "delete": {
                    "tags": ["auth"],
                    "summary": "PAT revoke (admin 스코프)",
                    "security": [{ "bearerAuth": [] }],
                    "parameters": [{ "name": "id", "in": "path", "required": true, "schema": { "type": "integer" } }],
                    "responses": ok_data("Revoked")
                }
            },
            "/api/v1/search": {
                "get": {
                    "tags": ["search"],
                    "summary": "전문 검색 (FTS5 trigram). 발행된 문서만.",
                    "security": [],
                    "parameters": [
                        { "name": "q", "in": "query", "required": true, "schema": { "type": "string" } },
                        { "name": "lang", "in": "query", "schema": { "type": "string" } },
                        { "name": "limit", "in": "query", "schema": { "type": "integer", "default": 20 } }
                    ],
                    "responses": ok_data("SearchHit[]")
                }
            },
            "/api/v1/blog/posts": {
                "get": { "tags": ["blog"], "summary": "발행본 목록 (?draft=true&lang=)", "security": [], "responses": ok_data("BlogPost[]") },
                "post": { "tags": ["blog"], "summary": "초안 생성 (post:write)", "security": [{ "bearerAuth": [] }], "responses": ok_data("BlogPost") }
            },
            "/api/v1/blog/posts/{slug}/publish": {
                "post": { "tags": ["blog"], "summary": "초안 발행 (post:publish)", "security": [{ "bearerAuth": [] }], "responses": ok_data("BlogPost") }
            },
            "/api/v1/projects": {
                "get": { "tags": ["projects"], "summary": "발행본 목록", "security": [], "responses": ok_data("Project[]") },
                "post": { "tags": ["projects"], "summary": "초안 생성 (post:write)", "security": [{ "bearerAuth": [] }], "responses": ok_data("Project") }
            },
            "/api/v1/links": {
                "get": { "tags": ["links"], "summary": "링크 목록", "security": [], "responses": ok_data("LinkCard[]") },
                "post": { "tags": ["links"], "summary": "링크 생성 (post:write, 즉시 발행)", "security": [{ "bearerAuth": [] }], "responses": ok_data("LinkCard") }
            },
            "/api/v1/novels": {
                "get": { "tags": ["novels"], "summary": "발행본 목록", "security": [], "responses": ok_data("Novel[]") },
                "post": { "tags": ["novels"], "summary": "소설 초안 생성 (post:write)", "security": [{ "bearerAuth": [] }], "responses": ok_data("Novel") }
            },
            "/api/v1/movies": {
                "get": { "tags": ["movies"], "summary": "발행본 목록", "security": [], "responses": ok_data("MovieEntry[]") },
                "post": { "tags": ["movies"], "summary": "영화 리뷰 초안 (post:write)", "security": [{ "bearerAuth": [] }], "responses": ok_data("MovieEntry") }
            },
            "/api/v1/books": {
                "get": { "tags": ["books"], "summary": "발행본 목록", "security": [], "responses": ok_data("Book[]") },
                "post": { "tags": ["books"], "summary": "도서 리뷰 초안 (post:write)", "security": [{ "bearerAuth": [] }], "responses": ok_data("Book") }
            },
            "/api/v1/scraps": {
                "get": { "tags": ["scraps"], "summary": "발행본 스크랩 목록", "security": [], "responses": ok_data("ScrapItem[]") },
                "post": { "tags": ["scraps"], "summary": "수동 스크랩 추가 (post:write)", "security": [{ "bearerAuth": [] }], "responses": ok_data("ScrapItem") }
            },
            "/api/v1/activity": {
                "get": { "tags": ["activity"], "summary": "최근 활동 목록 (읽기 전용 캐시)", "security": [], "responses": ok_data("ActivityEvent[]") }
            },
            "/api/v1/activity/webhook": {
                "post": { "tags": ["activity"], "summary": "GitHub webhook 수신 (공개, 서명 검증은 Phase 4+)", "security": [], "responses": { "200": { "description": "ok" } } }
            },
            "/healthz": {
                "get": { "tags": ["system"], "summary": "헬스체크", "security": [], "responses": { "200": { "description": "ok" } } }
            }
        }
    })
}

fn ok_data(type_name: &str) -> Value {
    json!({
        "200": {
            "description": "성공",
            "content": {
                "application/json": {
                    "schema": { "type": "object", "title": type_name }
                }
            }
        }
    })
}

/// Swagger UI CDN HTML. /api/v1/docs에서 서빙.
pub fn swagger_ui_html(spec_url: &str) -> String {
    const TPL: &str = r##"<!DOCTYPE html>
<html><head><meta charset="utf-8"><title>Oxipage API Docs</title>
<link rel="stylesheet" href="https://unpkg.com/swagger-ui-dist@5/swagger-ui.css">
</head><body>
<div id="swagger-ui"></div>
<script src="https://unpkg.com/swagger-ui-dist@5/swagger-ui-bundle.js"></script>
<script>
window.onload = function() {
  window.ui = SwaggerUIBundle({ url: "__SPEC_URL__", dom_id: "#swagger-ui" });
};
</script>
</body></html>"##;
    TPL.replace("__SPEC_URL__", spec_url)
}
