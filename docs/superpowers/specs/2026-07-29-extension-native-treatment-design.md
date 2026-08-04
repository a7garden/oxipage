# Setup Wizard / OpenAPI의 Extension Native Treatment 제거

- **상태**: 디자인 (구현 대기)
- **작성**: 2026-07-29
- **대상 버전**: v2 SSG (현재 작업 트리 기준)
- **우선순위**: High

## 1. 동기

Oxibuilder의 핵심 아키텍처 원칙은 **확장(`oxibuilder-ext-*`)이 자기 도메인의 시민**이라는 것이다 (doc/01 §1.2, §1.4).

```
코어가 아는 것: id, display_name, migrations, routes, lobby_summary, CLI, background_jobs, public_pages, table_names, on_startup, on_disable
코어가 모르는 것: 확장의 도메인 컬럼, UI 폼 필드, 외부 API 키 이름, 샘플 데이터
```

**Audit 결과, 두 곳에 native treatment가 새어 들어왔다**: setup 마법사(`oxibuilder-core/src/setup.rs` + `web/src/setup/`)와 OpenAPI 스펙(`oxibuilder-core/src/openapi.rs`).

### 1.1 Setup 마법사 위반 카탈로그

| 위치 | 문제 |
|------|------|
| `setup.rs:21-37` `SAMPLE_POST_*` 상수 | **blog 확장의 도메인 데이터**(환영 글 제목/슬러그/본문)가 코어에 하드코딩 |
| `setup.rs:529,537-548` `setup_content_handler` | `INSERT INTO blog_post …` — 코어가 직접 blog SQL 작성 |
| `setup.rs:378,461-475` `setup_site/profile_handler` | `UPDATE profile SET …` — 코어가 직접 profile SQL 작성 |
| `setup.rs:552-557` | `set_extension_config("movies","tmdb_key")` / `"books","aladin_key"` — 코어가 두 확장의 설정 키 이름 하드코딩 |
| `setup.rs:241-249` `setup_routes` | 8개 setup 엔드포인트 전부 코어가 정의 |
| `web/src/setup/StepProfile.tsx` | profile 입력 폼이 admin-web에 하드코딩. profile 비활성화해도 step이 뜸 |
| `web/src/setup/StepContent.tsx:63-75` | TMDB/알라딘 키 입력란이 admin-web에 하드코딩. movies/books 비활성화해도 뜸 |
| `web/src/setup/SetupWizard.tsx:25,119-176` | 5개 step이 switch-case로 하드코딩 |

**결과적 영향**:
- blog/movies/books 확장을 disable해도 setup wizard에서 환영 글/TMDB/알라딘 입력란이 그대로 노출.
- 확장을 추가/제거할 때마다 **코어 + admin-web 양쪽**을 손대야 한다 (다른 확장 부착은 한쪽이면 충분).
- doc/02 §2.13의 "확장 비활성화 시 라우트/잡/인덱스 즉시 정리"가 wizard UI에서는 무시됨.

### 1.2 OpenAPI 위반 카탈로그

| 위치 | 문제 |
|------|------|
| `openapi.rs:62-99` | 9개 확장의 `/api/console/{ext}/**` 경로가 코어의 OpenAPI 스펙에 **수동 등록**. 새 확장 추가 시 코어 수동 패치 필요. 주석에 "확장 라우트는 차후 `Extension::openapi_paths()` 훅으로 병합"이라고 명시 — 의도된 TODO이긴 하나 이 작업으로 마감. |

### 1.3 OpenAPI 외 — 발견된 다른 모든 케이스는 원칙 준수

CLI 서브커맨드(`cli_commands`), 라우트(`routes`), 로비 카드(`lobby_summary`), 백그라운드 잡(`background_jobs`), 마이그레이션(`migrations`), 빌드 파이프라인(`BuildExt`), search upsert, FTS 정리 — 전부 트레이트 경계 안에서 정상 동작.

---

## 2. 목표 / 비목표

### 2.1 목표
- **setup 마법사는 활성 확장 목록만 보고 step을 동적으로 조립**한다. profile/movies/books 비활성화 시 그 step이 사라진다.
- **모든 도메인 SQL과 샘플 데이터는 확장이 자기 트레이트 안에서 제공**한다. 코어는 "위임자"일 뿐이다.
- **OpenAPI 스펙은 레지스트리에서 동적으로 생성**된다. 새 확장 추가 시 코어 패치 불필요.
- 기존 UX(5-step 흐름, 외부 API 키 입력, 환영 글)를 보존한다.

### 2.2 비목표
- 1단계/2단계/3단계 위저드의 step 개수/이름 자유화는 범위 밖. 코어가 정한 순서: `site → extensions → {extension setup steps…} → theme → done`로 고정. 확장 setup step은 `extensions` 다음에 한꺼번에 끼워넣는다.
- WASM 확장의 setup hook은 v1 범위 밖. 컴파일된 확장만 자기 step을 등록한다 (WASM 확장은 `setup_wizard_step()`이 `None` 반환).
- 다국어 wizard UI (ko/en 토글 등) 자체는 범위 밖. 기존 한/영 혼재 유지.
- 다른 위젯(예: profile bio_ko/bio_en 같은 markdown 에디터) 추가는 안 함. profile은 단일 입력 필드 묶음만 노출.

---

## 3. 설계 — Extension 트레이트 확장

### 3.1 새 트레이트 메서드

`oxibuilder-core/src/extension.rs`의 `Extension` 트레이트에 세 가지 기본 메서드를 추가한다 (모두 기본은 "참여 안 함").

```rust
pub trait Extension: Send + Sync {
    // … 기존 메서드 …

    /// setup 마법사가 이 확장의 활성화 후 사용자에게 보여줄 setup step.
    /// None이면 step 없음 (대부분의 확장).
    fn setup_wizard_step(&self) -> Option<SetupStep> {
        None
    }

    /// 이 확장이 사용할 외부 API 키 메타. setup_status에 노출돼
    /// 마법사가 동적으로 키 입력란을 만든다. 실제 키 값은 setup_save_form으로 수신.
    fn external_api_keys(&self) -> Vec<ExternalApiKey> {
        Vec::new()
    }

    /// setup 완료 시점에 시드할 샘플 데이터 (예: 환영 글).
    /// `enabled` 가 false인 확장은 호출되지 않음.
    async fn seed_sample_data(&self, _ctx: &AppState) -> anyhow::Result<()> {
        Ok(())
    }
}
```

### 3.2 새 타입 — `oxibuilder-core/src/setup.rs` (또는 `extension.rs`로 분리)

```rust
/// setup wizard 한 step의 선언적 정의.
/// 코어가 step 라우팅 + 폼 디스패치를 담당하고, 이 구조체가 UI 필드와 저장 콜백을 표현.
#[derive(Debug, Clone, Serialize)]
pub struct SetupStep {
    pub id: String,                  // "profile", "blog_sample" 등. /setup/step/{id} 라우팅 키.
    pub title_ko: String,
    pub title_en: String,
    pub description_ko: String,
    pub description_en: String,
    pub fields: Vec<SetupField>,     // 폼 필드 목록 (input)
    /// 확장 구현이 처리하는 form 저장 핸들러. 코어가 호출만 한다.
    pub save_handler: Arc<dyn SetupSaveHandler>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SetupField {
    pub name: String,                // JSON key (예: "display_name", "tagline_ko")
    pub label_ko: String,
    pub label_en: String,
    pub kind: SetupFieldKind,
    pub required: bool,
    pub placeholder_ko: Option<String>,
    pub placeholder_en: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SetupFieldKind {
    Text,         // <input type="text">
    Textarea,     // <textarea> (markdown bio 등)
    Url,
}

/// 코어가 form JSON을 받아 위임. 확장이 자기 DB에 쓴다.
#[async_trait]
pub trait SetupSaveHandler: Send + Sync {
    /// form: 클라이언트가 보낸 필드 값 맵. unknown 필드는 무시하고 known만 처리.
    async fn save(
        &self,
        ctx: &AppState,
        form: &serde_json::Map<String, serde_json::Value>,
    ) -> anyhow::Result<()>;
}

/// 외부 API 키 한 줄. 마법사가 input을 그리고 save 시 확장이 env var에 저장.
#[derive(Debug, Clone, Serialize)]
pub struct ExternalApiKey {
    pub id: String,                  // "tmdb_key", "aladin_key", "github_username" 등
    pub label_ko: String,
    pub label_en: String,
    pub env_var: String,             // "OXIBUILDER_TMDB_KEY" — 저장 시 process env에도 세팅
    pub required: bool,              // false면 "선택"
    /// 키 값을 어떤 식으로 보존할지. env_var만, 또는 env_var + extension_state.config JSON
    pub scope: ExternalKeyScope,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalKeyScope {
    EnvOnly,                 // process env에 set (현재 IntegrationsConfig.tmdb_key() 패턴)
    ExtensionConfig,         // env + extension_state.config JSON 둘 다 (setup 현재 동작)
}
```

### 3.3 `/api/console/setup/status` 응답 확장

```rust
pub struct StatusResult {
    pub setup_mode: bool,
    pub completed_steps: Vec<String>,
    pub available_extensions: Vec<ExtInfo>,
    pub available_themes: Vec<ThemeEntry>,
    // 신규:
    pub extension_steps: Vec<ExtensionStepInfo>,   // 활성 확장만
    pub external_api_keys: Vec<ExternalApiKey>,    // 활성 확장이 노출한 모든 키
}

pub struct ExtensionStepInfo {
    pub id: String,
    pub title_ko: String,
    pub title_en: String,
    pub description_ko: String,
    pub description_en: String,
    pub fields: Vec<SetupField>,
}
```

`extension_steps` 순서는 **활성화된 확정의 레지스트리 순서** (`registry.iter()` 순). wizard가 한 step당 한 칸씩 그린다.

### 3.4 신규 라우트

기존 8개 → **3개 동적 라우트**로 축소:

| 메서드 | 경로 | 동작 |
|--------|------|------|
| `GET` | `/api/console/setup/status` | (위 §3.3 응답) |
| `POST` | `/api/console/setup/extension-step/{id}` | `{id}`에 해당하는 확장의 `SetupSaveHandler::save`로 위임 |
| `POST` | `/api/console/setup/external-keys` | body에 `{ "values": {"tmdb_key":"…","aladin_key":"…",…} }` 받아 활성 확장의 키별로 env+config 저장 |

제거되는 라우트 (기존): `/setup/site`(theme 외 site는 코어 step으로 유지), `/setup/admin`, `/setup/extensions`, `/setup/profile`, `/setup/theme`, `/setup/content`, `/setup/complete`. 새 매핑:

- `/setup/site` — 유지 (코어 자체 step. site_name/base_url은 코어 책임).
- `/setup/extensions` — 유지 (enabled 토글은 코어 책임).
- `/setup/extension-step/{id}` — 신규. 확장 step 저장.
- `/setup/external-keys` — 신규. 활성 확장이 노출한 키 일괄 저장.
- `/setup/theme` — 유지 (테마는 코어 책임).
- `/setup/complete` — 유지. `seed_sample_data`는 여기서 활성 확장 전부에 호출.

### 3.5 마법사 UI 동적 조립

`web/src/setup/SetupWizard.tsx`의 switch-case를 다음으로 교체:

```tsx
const steps = useMemo(() => {
  const base = [
    { type: "site",  label: "사이트" },
    { type: "extensions", label: "확장 선택" },
  ];
  const extSteps = (status?.extension_steps ?? []).map(s => ({
    type: "extension-step",
    id: s.id,
    label: s.title_ko,
  }));
  return [
    ...base,
    ...extSteps,
    { type: "external-keys", label: "외부 API 키" },  // 외부 키가 있을 때만
    { type: "theme", label: "테마 & 레이아웃" },
    { type: "done",  label: "완료" },
  ];
}, [status]);
```

각 step은 `<GenericStep>`가 폼 필드를 보고 자동으로 input/textarea를 그림:

- `SetupField.kind == Text` → `<Input>`
- `SetupField.kind == Textarea` → `<Textarea>`
- `SetupField.kind == Url` → `<Input type="url">`

**파일 제거/대체**:
- `web/src/setup/StepProfile.tsx` — 삭제 (profile 확장의 `setup_wizard_step()`이 동일 폼 제공).
- `web/src/setup/StepContent.tsx` — 삭제 (external-keys step + blog 확장의 `seed_sample_data()`로 분리).

**파일 신규**:
- `web/src/setup/GenericStep.tsx` — `SetupField[]` 받아 동적 폼 렌더.
- `web/src/setup/ExternalKeysStep.tsx` — `ExternalApiKey[]` 받아 키 입력란들 렌더.

### 3.6 OpenAPI 동적 생성

`openapi.rs::openapi_spec()` 시그니처를 `openapi_spec(base_url, registry: &ExtensionRegistry)`로 바꾸고, `state.registry.iter()`로 코어/확장 경로를 한 번에 조립한다. 코어 시스템 경로(`/healthz`, `/api/console/setup/**`, `/api/console/lobby/manifest` 등)는 그대로, **확장 경로는 registry를 순회해 `ext.id()`별로 자동 등록**:

```rust
fn extension_paths(ext: &dyn Extension) -> Value {
    json!({
        format!("/api/console/{}/", ext.id()): {
            "get": { "tags": [ext.id()], "summary": format!("{} 목록", ext.display_name(Lang::En)),
                     "security": [], "responses": ok_data("[]") },
            "post": { "tags": [ext.id()], "summary": format!("{} 생성 (post:write)"),
                      "security": [{ "bearer": [] }], "responses": ok_data("{}") }
        },
        format!("/api/console/{}/{{slug}}", ext.id()): {
            "get": { "tags": [ext.id()], "summary": "단건 조회", "security": [], "responses": ok_data("{}") },
            "patch": { "tags": [ext.id()], "summary": "수정 (post:write)", "security": [{ "bearer": [] }] },
            "delete": { "tags": [ext.id()], "summary": "삭제", "security": [{ "bearer": [] }] }
        },
        format!("/api/console/{}/{{slug}}/publish", ext.id()): {
            "post": { "tags": [ext.id()], "summary": "발행 (post:publish)", "security": [{ "bearer": [] }] }
        }
    })
}
```

`http.rs::build_app`에서 `state`를 받아 `build_app(state)` → `openapi_spec(base_url, &state.registry)`로 호출 변경. registry가 비어있는 단위 테스트용 헬퍼(`openapi_spec_with_extensions(base_url, ext_paths: Value)`)는 내부 helper로 유지.

**알려진 한계**: 응답 스키마는 `{}` / `[]` placeholder 수준. 정확한 모델은 utoipa 도입 등 별도 작업의 영역. 이번 작업의 범위는 "확장 추가 시 코어 패치 없이 자동 등록"까지만.

### 3.7 Setup 완료 시 시드

`setup_complete_handler`에서 활성 확장 순회하며 `seed_sample_data(&state)` 호출:

```rust
for ext in state.registry.iter() {
    if state.registry.is_active(ext.id()).await {
        if let Err(e) = ext.seed_sample_data(&state).await {
            tracing::warn!(extension = ext.id(), error = %e, "seed_sample_data failed");
            // best-effort: 실패해도 setup 완료는 진행
        }
    }
}
```

`blog` 확장의 `seed_sample_data`가 자기 DB에 환영 글을 직접 INSERT. 코어는 더 이상 `INSERT INTO blog_post`를 모른다.

### 3.8 외부 키 저장 패턴

`set_extension_config`는 유지하되 호출자는 바뀌어, `ExternalKeyScope::ExtensionConfig`인 키만 `extension_state.config` JSON에 추가 기록. `EnvOnly`인 키는 `std::env::set_var`만 (현재 tmdb/aladin은 환경변수만 쓰므로 실제로는 EnvOnly로 충분하지만 기존 호환을 위해 ExtensionConfig로 시작).

→ `setup_external_keys_handler`는 `external_api_keys()`를 registry에서 모아 ID별로 분기. 각 키의 `id`가 `"tmdb_key"`이면 `MoviesExtension`의 `setup_save_external_key(&state, "tmdb_key", value)`가 정의돼 호출되는 게 이상. **단, 이건 키 ID를 코어가 알아야 한다는 점에서는 여전히 native treatment가 새는 것**.

→ **더 정직한 모델**: `Extension`에 `save_external_key` 트레이트 메서드를 추가하는 것이 일관됨:

```rust
#[async_trait]
pub trait Extension: Send + Sync {
    // …

    /// external_api_keys()에 등록된 키 값을 저장. 코어가 id로 디스패치.
    /// 기본 구현: env_var가 있으면 std::env::set_var + extension_state.config JSON에 저장.
    async fn save_external_key(
        &self,
        ctx: &AppState,
        key_id: &str,
        value: &str,
    ) -> anyhow::Result<()> {
        // 기본 impl — key_id에 해당하는 ExternalApiKey를 찾아 env + config JSON 저장.
        // 확장이 override해 자기 도메인별 추가 검증/저장 가능.
        for k in self.external_api_keys() {
            if k.id == key_id {
                std::env::set_var(&k.env_var, value);
                set_extension_config(ctx, self.id(), &k.env_var, value).await?;
                return Ok(());
            }
        }
        anyhow::bail!("unknown external key: {key_id}");
    }
}
```

→ 이렇게 하면 코어는 키 ID를 모른다. `tmdb_key`라는 문자열 자체는 `MoviesExtension::external_api_keys()` 안에서만 등장.

### 3.9 디스패치 라우터

`extension-step` 라우트 핸들러:

```rust
async fn setup_extension_step_handler(
    State(state): State<AppState>,
    Path(step_id): Path<String>,
    Json(form): Json<serde_json::Map<String, serde_json::Value>>,
) -> Result<Json<DataEnvelope<SimpleOk>>, ApiError> {
    for ext in state.registry.iter() {
        if let Some(step) = ext.setup_wizard_step() {
            if step.id == step_id {
                step.save_handler.save(&state, &form).await?;
                return Ok(Json(DataEnvelope { data: SimpleOk { ok: true } }));
            }
        }
    }
    Err(ApiError::new(StatusCode::NOT_FOUND, "unknown_step", "no such extension setup step"))
}
```

`external-keys` 라우트 핸들러:

```rust
async fn setup_external_keys_handler(
    State(state): State<AppState>,
    Json(body): Json<ExternalKeysInput>,
) -> Result<Json<DataEnvelope<SimpleOk>>, ApiError> {
    // body.values: {"<key_id>": "<value>"} — 클라이언트가 활성 확장의 키 id만 보냄
    for ext in state.registry.iter() {
        if !state.registry.is_active(ext.id()).await { continue; }
        for k in ext.external_api_keys() {
            if let Some(v) = body.values.get(&k.id) {
                if let Some(s) = v.as_str() {
                    ext.save_external_key(&state, &k.id, s).await?;
                }
            }
        }
    }
    Ok(Json(DataEnvelope { data: SimpleOk { ok: true } }))
}
```

---

## 4. 데이터 흐름

### 4.1 Setup 완료 시점

```
1. boot → is_setup_needed() → setup_mode=true
2. /api/console/setup/status
   GET → registry.iter() → 각 ext.setup_wizard_step() + ext.external_api_keys()
   ← JSON { extension_steps, external_api_keys, ... }
3. UI: 동적 step 조립 + 렌더
4. 사용자 입력 + POST /api/console/setup/extension-step/{id}
   → registry 디스패치 → step.save_handler.save(&state, form)
   → 확장이 자기 DB/테이블에 쓴다
5. (외부 키가 있으면) POST /api/console/setup/external-keys
   → registry 디스패치 → ext.save_external_key() per id
6. POST /api/console/setup/theme (변경 없음)
7. POST /api/console/setup/complete
   → 활성 ext.seed_sample_data(&state) 일괄 호출
   → setup_state.setup_completed_at = now
   → setup API 410 Gone
```

### 4.2 새 확장 추가 시 (참고)

1. `oxibuilder-ext-X/Cargo.toml` + 마이그레이션 + routes
2. `crates/oxibuilder-console/src/lib.rs`의 `all_extensions()` vec에 한 줄 추가
3. (선택) `impl Extension for XExtension { fn setup_wizard_step() -> Some(SetupStep { ... }) }`
4. (선택) `impl Extension for XExtension { fn external_api_keys() -> vec![...] }`
5. (선택) `impl Extension for XExtension { async fn seed_sample_data(...) }`

코어 + admin-web **수정 없음**. OpenAPI 스펙에도 자동으로 등장.

---

## 5. 호환성 / 마이그레이션

### 5.1 기존 setup 핸들러

- `/setup/profile`, `/setup/content`, `/setup/admin` 엔드포인트 제거. 단, `setup_admin`은 어차피 no-op이므로 무영향.
- `setup_extensions` 유지 (enabled 토글은 코어 책임).
- `setup_site`, `setup_theme`, `setup_complete` 유지.
- 클라이언트 `web/src/setup/api.ts`에서 `submitProfile`, `submitContent` 함수 제거. `submitExtensions`/`submitSite`/`submitTheme`/`submitComplete` 유지 + 신규 `submitExtensionStep(id, form)` / `submitExternalKeys(values)` 추가.

### 5.2 기존 DB 스키마

변경 없음. `setup_state`, `profile`, `blog_post`, `extension_state.config` 모두 그대로. profile/blog 확장의 자기 API(`PATCH /api/console/profile`, `POST /api/console/blog/posts`)는 별도 라우트로 이미 존재 — 위자드 setup과 무관.

### 5.3 기존 테스트

- `crates/oxibuilder-ext-profile/tests/api.rs` — 영향 없음. profile API 라우트는 그대로.
- `crates/oxibuilder-ext-blog/tests/api.rs` — 영향 없음.
- `crates/oxibuilder-core/src/setup.rs` (현재 테스트 없음) — 신규 테스트 추가:
  - `extension_step_dispatch_routes_to_correct_handler`
  - `external_keys_dispatch_to_correct_extension`
  - `seed_sample_data_called_only_for_active_extensions`
  - `inactive_extension_step_excluded_from_status`

---

## 6. 보안 / 안전성

- `setup_gate` 미들웨어 (loopback-only + 완료 후 410)는 그대로. 새 라우트도 동일 게이트 적용.
- `save_external_key`의 기본 impl이 `std::env::set_var`을 호출. process env 오염은 현재 동작과 동일 (setup 완료 후 영구). 위험 없음.
- `seed_sample_data`는 best-effort, 실패해도 setup 완료 진행 (tracing warn만).
- form JSON 검증은 확장이 자기 `save`에서 책임. 코드는 패스스루.

---

## 7. 영향 받는 파일 / 모듈

### 7.1 변경

| 파일 | 변경 |
|------|------|
| `crates/oxibuilder-core/src/extension.rs` | 트레이트에 `setup_wizard_step`, `external_api_keys`, `seed_sample_data`, `save_external_key` 추가. `SetupStep`/`SetupField`/`SetupFieldKind`/`SetupSaveHandler`/`ExternalApiKey`/`ExternalKeyScope` 타입 추가 |
| `crates/oxibuilder-core/src/setup.rs` | `setup_status_handler` 응답에 `extension_steps`, `external_api_keys` 추가. `/setup/extension-step/{id}` + `/setup/external-keys` 라우트 추가. `setup_profile_handler`, `setup_content_handler` 제거. `setup_complete_handler`에 seed 호출 추가 |
| `crates/oxibuilder-core/src/openapi.rs` | `openapi_spec` 시그니처에 `&ExtensionRegistry` 추가. 경로 자동 조립 |
| `crates/oxibuilder-core/src/http.rs` | `build_app`에서 `openapi_spec(base_url, &state.registry)` 호출 |
| `crates/oxibuilder-ext-profile/src/lib.rs` | `setup_wizard_step()` 구현 (display_name/tagline_ko/tagline_en/github_username + bio_ko/bio_en). `save_external_key`는 사용 안 함 (github_username은 profile 필드) |
| `crates/oxibuilder-ext-blog/src/lib.rs` | `seed_sample_data()` 구현 (환영 글 INSERT) |
| `crates/oxibuilder-ext-movies/src/lib.rs` | `external_api_keys()`에 `tmdb_key` 노출. `save_external_key`는 기본 impl (env + config) |
| `crates/oxibuilder-ext-books/src/lib.rs` | `external_api_keys()`에 `aladin_key` 노출. 동일 |
| `crates/oxibuilder-ext-activity/src/lib.rs` | `external_api_keys()`에 `github_username`(EnvOnly) 노출 |
| `web/src/setup/SetupWizard.tsx` | step 조립을 status 응답 기반으로 변경 |
| `web/src/setup/SetupWizard.tsx` (StepProfile/StepContent 호출 제거) | 동적 step으로 |
| `web/src/setup/GenericStep.tsx` | 신규. `SetupField[]` 받아 input 렌더 |
| `web/src/setup/ExternalKeysStep.tsx` | 신규. `ExternalApiKey[]` 렌더 |
| `web/src/setup/api.ts` | `submitExtensionStep`, `submitExternalKeys` 추가. `submitProfile`/`submitContent` 제거 |
| `web/src/setup/StepProfile.tsx` | 삭제 |
| `web/src/setup/StepContent.tsx` | 삭제 |

### 7.2 신규 문서

- `docs/extension-sdk.md` — § "Setup wizard hooks" 추가
- `doc/13-first-run-ux.md` §13.7.2 — step 조립이 동적임을 반영. §13.5.2 API 표 갱신
- `doc/01-architecture.md` §1.4 — `Extension` 트레이트 의사코드 갱신

### 7.3 영향 없음

- 모든 확장 routes/repo/model (자기 도메인 그대로)
- BuildExt, CLI 서브커맨드, 백그라운드 잡, lobby summary, FTS, search
- 모든 마이그레이션 SQL
- 공개 사이트 빌드 산출물
- deploy/build/cache refresh/serve 등 다른 명령

---

## 8. 검증 기준

1. `cargo test --workspace` — 163개 기존 + 4개 신규 테스트 모두 통과
2. `cargo clippy --workspace --all-targets -- -D warnings` — 깨끗
3. `cd web && bun run build` — 깨끗
4. **수동 스모크**:
   - `profile`만 활성화한 상태에서 setup → profile step만 보이고 movies/books 키 step 안 뜸
   - `movies`/`books`만 활성화한 상태에서 setup → 환영 글 step이 사라지고 키 step만 뜸
   - `blog`/`profile` 비활성화 → setup 완료 후 DB에 blog_post 행 없음, profile은 빈 singleton
5. OpenAPI 스펙(`/api/console/docs/openapi.json`)에 레지스트리 9개 확장의 경로가 모두 자동 등장

---

## 9. 대안 검토 (간단히)

| 대안 | 기각 이유 |
|------|-----------|
| setup wizard를 확장이 라우트와 핸들러를 통째로 등록하게 (`setup_routes`) | 동적 라우팅 = WASM 호환 고민. v2 SSG 모델에서는 단순 디스패치로 충분. |
| profile/blog 확장에 setup_handler 함수만 추가 (UI는 코어가 그림) | UI 필드 정의가 코어에 남는다. 진정한 분리는 UI 정의까지 확장이 가져야. |
| step 개수/이름 자유화 | YAGNI. 사이트/확장선택/확장step들/외부키/테마/완료 고정. |
| 각 확장이 자기 React chunk에 자기 wizard step을 export | 런타임 동적 import는 가능하나 v1에서 과한 분기. 단일 SPA 안에서 `GenericStep`이 충분. |

---

## 10. 단계 요약

1. 트레이트 확장 + 타입 정의
2. setup 핸들러 디스패치 + status 응답 확장
3. profile/blog/movies/books/activity에 hook 구현
4. openapi 동적 생성
5. web wizard 동적 step 조립
6. 테스트 + 문서 + 빌드
