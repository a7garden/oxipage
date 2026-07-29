//! OpenAPI 스펙 (doc/04 §4.5).
//!
//! **편차:** `utoipa` 자동 생성 대신 수동 `serde_json` 스펙. 의존성 절약 + 코어
//! 라우트를 즉시 문서화. 확장 라우트는 `ExtensionRegistry`에서 동적으로 조립
//! — 새 확장 추가 시 코어 패치 불필요.
//! `/api/console/docs/openapi.json` + `/api/console/docs`(Swagger UI CDN).

use crate::extension::Extension;
use crate::registry::ExtensionRegistry;
use serde_json::{Value, json};

pub fn openapi_spec(base_url: &str, registry: &ExtensionRegistry) -> Value {
    let server = if base_url.is_empty() {
        "/".to_string()
    } else {
        format!("{}/", base_url.trim_end_matches('/'))
    };
    let mut paths = core_paths();
    for ext in registry.iter() {
        merge_extension_paths(&mut paths, ext.as_ref());
    }
    json!({
        "openapi": "3.0.3",
        "info": {
            "title": "Oxipage API",
            "version": "0.1.0",
            "description": "개인 창작 작업실 홈페이지 API (doc/04 §4.5). 모든 엔드포인트 공개 — 로컬 관리 서버 전용.",
            "license": { "name": "MIT" }
        },
        "servers": [{ "url": server }],
        "paths": paths
    })
}

/// 코어 시스템 경로 (lobby/manifest, search, lobby/config, healthz, backup 등).
/// 확장은 별도로 registry에서 조립한다.
fn core_paths() -> Value {
    json!({
        "/api/console/lobby/manifest": {
            "get": {
                "tags": ["lobby"],
                "summary": "로비 매니페스트 (site + extensions + per-extension lobby config)",
                "security": [],
                "responses": ok_data("Manifest")
            }
        },
        "/api/console/lobby/config": {
            "get": {
                "tags": ["lobby"],
                "summary": "모든 확장의 로비 표시 설정",
                "security": [],
                "responses": ok_data("LobbyConfig[]")
            }
        },
        "/api/console/lobby/config/{ext_id}": {
            "put": {
                "tags": ["lobby"],
                "summary": "확장별 로비 표시 모드/순서 갱신 (post:write)",
                "security": [],
                "parameters": [{ "name": "ext_id", "in": "path", "required": true, "schema": { "type": "string" } }],
                "responses": ok_data("LobbyConfig")
            }
        },
        "/api/console/search": {
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
        "/api/console/backup/snapshot": {
            "post": {
                "tags": ["system"],
                "summary": "SQLite VACUUM INTO 백업 스냅샷 (admin)",
                "security": [{ "bearer": [] }],
                "responses": ok_data("BackupSnapshot")
            }
        },
        "/api/console/cache/refresh": {
            "post": {
                "tags": ["system"],
                "summary": "외부 API 캐시 갱신 (TMDB/Books/Activity)",
                "security": [],
                "responses": { "200": { "description": "ok" } }
            }
        },
        "/api/console/setup/status": {
            "get": {
                "tags": ["setup"],
                "summary": "setup 모드 상태 (extension_wizards 동적 조립)",
                "security": [],
                "responses": ok_data("SetupStatus")
            }
        },
        "/api/console/setup/site": {
            "post": {
                "tags": ["setup"],
                "summary": "사이트 이름/URL 설정 (loopback-only)",
                "security": [],
                "responses": ok_data("SimpleOk")
            }
        },
        "/api/console/setup/extensions": {
            "post": {
                "tags": ["setup"],
                "summary": "활성화할 확장 목록 (loopback-only)",
                "security": [],
                "responses": ok_data("SimpleOk")
            }
        },
        "/api/console/setup/extension-step/{ext_id}/{step_id}": {
            "post": {
                "tags": ["setup"],
                "summary": "확장 정의 setup step 저장 (registry 디스패치, loopback-only)",
                "security": [],
                "parameters": [{ "name": "ext_id", "in": "path", "required": true, "schema": { "type": "string" } }, { "name": "step_id", "in": "path", "required": true, "schema": { "type": "string" } }],
                "responses": ok_data("SimpleOk")
            }
        },
        "/api/console/setup/theme": {
            "post": {
                "tags": ["setup"],
                "summary": "테마/레이아웃 설정 (loopback-only)",
                "security": [],
                "responses": ok_data("SimpleOk")
            }
        },
        "/api/console/setup/complete": {
            "post": {
                "tags": ["setup"],
                "summary": "setup 완료 (활성 확장의 seed_sample_data 호출)",
                "security": [],
                "responses": ok_data("CompleteResult")
            }
        },
        "/healthz": {
            "get": {
                "tags": ["system"],
                "summary": "헬스체크",
                "security": [],
                "responses": { "200": { "description": "ok" } }
            }
        }
    })
}

/// 단일 확장의 OpenAPI 경로 조립. 패턴:
/// `/api/console/{id}/`            — GET 목록, POST 초안 생성
/// `/api/console/{id}/{slug}`      — GET/PATCH/DELETE 단건
/// `/api/console/{id}/{slug}/publish` — POST 발행
///
/// `display_name(Lang::En)`을 summary에 사용해 사람이 읽기 좋게 만든다.
fn merge_extension_paths(paths: &mut Value, ext: &dyn Extension) {
    let id = ext.id();
    let name = ext.display_name(crate::extension::Lang::En);

    let base = format!("/api/console/{id}/");
    let item = format!("/api/console/{id}/{{slug}}");
    let publish = format!("/api/console/{id}/{{slug}}/publish");

    let list_create = json!({
        "get": {
            "tags": [id],
            "summary": format!("{name} list"),
            "security": [],
            "responses": ok_data("[]")
        },
        "post": {
            "tags": [id],
            "summary": format!("{name} create (post:write)"),
            "security": [{ "bearer": [] }],
            "responses": ok_data("{}")
        }
    });

    let item_obj = json!({
        "get": {
            "tags": [id],
            "summary": format!("{name} show"),
            "security": [],
            "parameters": [{ "name": "slug", "in": "path", "required": true, "schema": { "type": "string" } }],
            "responses": ok_data("{}")
        },
        "patch": {
            "tags": [id],
            "summary": format!("{name} update (post:write)"),
            "security": [{ "bearer": [] }],
            "parameters": [{ "name": "slug", "in": "path", "required": true, "schema": { "type": "string" } }],
            "responses": ok_data("{}")
        },
        "delete": {
            "tags": [id],
            "summary": format!("{name} delete"),
            "security": [{ "bearer": [] }],
            "parameters": [{ "name": "slug", "in": "path", "required": true, "schema": { "type": "string" } }],
            "responses": { "200": { "description": "ok" } }
        }
    });

    let publish_obj = json!({
        "post": {
            "tags": [id],
            "summary": format!("{name} publish (post:publish)"),
            "security": [{ "bearer": [] }],
            "parameters": [{ "name": "slug", "in": "path", "required": true, "schema": { "type": "string" } }],
            "responses": ok_data("{}")
        }
    });

    let map = paths.as_object_mut().expect("paths is a JSON object");
    map.insert(base, list_create);
    map.insert(item, item_obj);
    map.insert(publish, publish_obj);
}

fn ok_data(type_name: &str) -> Value {
    json!({
        "200": {
            "description": "ok",
            "content": {
                "application/json": {
                    "schema": { "$ref": "#/components/schemas/Data" }
                }
            }
        },
        "_type_hint": type_name
    })
}

/// Swagger UI CDN HTML. /api/console/docs에서 서빙.
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
