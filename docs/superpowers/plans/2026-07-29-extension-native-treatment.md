# Extension Native Treatment 제거 — 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use subagent-driven-development.
> Steps use checkbox (`- [ ]`) syntax for tracking.

**Architecture:** `Extension` 트레이트에 4개 hook 추가 (`setup_wizard_step`, `external_api_keys`, `seed_sample_data`, `save_external_key`). 코어 setup 핸들러는 registry 디스패치로 단순화. OpenAPI 스펙은 registry 순회로 동적 생성. web 위저드는 status 응답으로 step 동적 조립.

**Tech Stack:** Rust (axum 0.8, sqlx), React 19 + Vite 7 + TypeScript

**Design doc:** `docs/superpowers/specs/2026-07-29-extension-native-treatment-design.md`

---

## Phase 0 — 사전 확인

- [ ] **Step 0.1:** `git status` 깨끗한지, `cargo test --workspace` baseline 통과 (163 tests) 확인
- [ ] **Step 0.2:** 변경 대상 파일 트리 캐시. 각 파일의 현재 라인 수를 기록해 회귀 비교 기준으로 둠
  - `crates/oxipage-core/src/extension.rs`
  - `crates/oxipage-core/src/setup.rs`
  - `crates/oxipage-core/src/openapi.rs`
  - `crates/oxipage-core/src/http.rs`
  - `crates/oxipage-ext-profile/src/lib.rs`
  - `crates/oxipage-ext-blog/src/lib.rs`
  - `crates/oxipage-ext-movies/src/lib.rs`
  - `crates/oxipage-ext-books/src/lib.rs`
  - `crates/oxipage-ext-activity/src/lib.rs`
  - `web/src/setup/SetupWizard.tsx`
  - `web/src/setup/api.ts`

---

## Phase 1 — Core: Extension 트레이트 확장

목표: 트레이트에 4개 hook + 관련 타입 추가. 기존 Extension 구현체 9개는 모두 컴파일 통과 (기본 impl 사용).

### Task P1.1: `Extension` 트레이트에 hook 추가

**Files:**
- Modify: `crates/oxipage-core/src/extension.rs`

**Changes:**
1. 트레이트 `Extension` 본문 끝에 4개 메서드 추가 (모두 기본 impl):
   - `fn setup_wizard_step(&self) -> Option<SetupStep>` — `None`
   - `fn external_api_keys(&self) -> Vec<ExternalApiKey>` — `vec![]`
   - `async fn seed_sample_data(&self, _ctx: &AppState) -> anyhow::Result<()>` — `Ok(())`
   - `async fn save_external_key(&self, ctx, key_id, value) -> anyhow::Result<()>` — `self.external_api_keys()` 순회해 일치하는 키가 있으면 `std::env::set_var(&k.env_var, value)` + `set_extension_config(ctx, self.id(), &k.env_var, value)`. 아니면 `bail!`
2. 트레이트와 같은 파일에 새 타입들 추가:
   - `pub struct SetupStep { id, title_ko, title_en, description_ko, description_en, fields, save_handler }`
   - `pub struct SetupField { name, label_ko, label_en, kind, required, placeholder_ko, placeholder_en }`
   - `#[serde(tag = "type", rename_all = "snake_case")] pub enum SetupFieldKind { Text, Textarea, Url }`
   - `#[async_trait] pub trait SetupSaveHandler: Send + Sync { async fn save(&self, ctx, form) -> anyhow::Result<()> }`
   - `pub struct ExternalApiKey { id, label_ko, label_en, env_var, required, scope }`
   - `#[serde(rename_all = "snake_case")] pub enum ExternalKeyScope { EnvOnly, ExtensionConfig }`
3. `set_extension_config`는 이미 `setup.rs`에 존재하지만 trait 기본 impl에서 호출하려면 `extension.rs`가 아닌 `setup.rs`에 있음. 트레이트 기본 impl은 `setup::set_extension_config(&ctx, ...)`을 호출하도록 한다.

**Acceptance:**
- `cargo build -p oxipage-core` 성공
- 모든 기존 9개 확장 `cargo build --workspace` 성공 (트레이트 기본 impl만 사용하면 됨)
- `cargo clippy -p oxipage-core --all-targets -- -D warnings` 깨끗

---

## Phase 2 — Core: setup 핸들러 디스패치 + status 응답 확장

목표: `/api/console/setup/status`가 활성 확장의 step + 키 목록을 응답. `/setup/extension-step/{id}`와 `/setup/external-keys` 신규 라우트. `setup_profile`/`setup_content` 핸들러 삭제.

### Task P2.1: setup 응답 확장 + 핸들러 리팩토링

**Files:**
- Modify: `crates/oxipage-core/src/setup.rs`

**Changes:**
1. `StatusResult`에 `pub extension_steps: Vec<ExtensionStepInfo>`, `pub external_api_keys: Vec<ExternalApiKey>` 추가.
2. `ExtensionStepInfo` 신규 타입: `pub struct ExtensionStepInfo { id, title_ko, title_en, description_ko, description_en, fields }`. `SetupField`는 그대로 재사용.
3. `setup_status_handler`에서 `state.registry.iter()`를 순회. `is_active(ext.id()).await`인 확장의 `setup_wizard_step()`를 모아 `ExtensionStepInfo` vec으로 변환. `external_api_keys()`는 활성 확장 전체에서 평탄화해 dedup-by-id로 모음.
4. `setup_routes` 함수 갱신:
   - 제거: `/setup/profile`, `/setup/content`. (`/setup/admin`도 함께 제거 가능 — 어차피 no-op)
   - 추가: `POST /setup/extension-step/{id}` → `setup_extension_step_handler`
   - 추가: `POST /setup/external-keys` → `setup_external_keys_handler`
5. `setup_profile_handler`, `setup_content_handler` 삭제.
6. `setup_complete_handler`에서 seed 호출 추가:
   ```rust
   for ext in state.registry.iter() {
       if state.registry.is_active(ext.id()).await {
           if let Err(e) = ext.seed_sample_data(&state).await {
               tracing::warn!(extension = ext.id(), error = %e, "seed_sample_data failed");
           }
       }
   }
   ```
7. `get_completed_steps`에서 `profile`/`content` 항목 제거 (이제 동적 step이므로 core가 추적 불가 — `completed_steps` 자체를 비활성화하거나 동적 step id로 채움).
8. 신규 핸들러:
   ```rust
   pub async fn setup_extension_step_handler(
       State(state): State<AppState>,
       Path(step_id): Path<String>,
       Json(form): Json<serde_json::Map<String, serde_json::Value>>,
   ) -> Result<Json<DataEnvelope<SimpleOk>>, ApiError> {
       for ext in state.registry.iter() {
           if let Some(step) = ext.setup_wizard_step() {
               if step.id == step_id {
                   step.save_handler.save(&state, &form).await
                       .map_err(|e| ApiError::internal(e))?;
                   return Ok(Json(DataEnvelope { data: SimpleOk { ok: true } }));
               }
           }
       }
       Err(ApiError::new(StatusCode::NOT_FOUND, "unknown_step", "no such extension setup step"))
   }

   #[derive(Deserialize)]
   pub struct ExternalKeysInput { #[serde(default)] pub values: serde_json::Map<String, serde_json::Value> }

   pub async fn setup_external_keys_handler(
       State(state): State<AppState>,
       Json(body): Json<ExternalKeysInput>,
   ) -> Result<Json<DataEnvelope<SimpleOk>>, ApiError> {
       for ext in state.registry.iter() {
           if !state.registry.is_active(ext.id()).await { continue; }
           for k in ext.external_api_keys() {
               if let Some(v) = body.values.get(&k.id) {
                   if let Some(s) = v.as_str() {
                       ext.save_external_key(&state, &k.id, s).await
                           .map_err(ApiError::internal)?;
                   }
               }
           }
       }
       Ok(Json(DataEnvelope { data: SimpleOk { ok: true } }))
   }
   ```
9. `SAMPLE_POST_TITLE/SLUG/BODY` 상수 제거.

**Acceptance:**
- `cargo build -p oxipage-core` 성공
- `cargo test -p oxipage-core` 성공 (기존 setup 테스트 없음 — 다만 setup_status에서 동적 step이 잘 노출되는지 새 테스트 추가)
- `cargo clippy -p oxipage-core --all-targets -- -D warnings` 깨끗

### Task P2.2: setup 통합 테스트 추가

**Files:**
- Create: `crates/oxipage-core/tests/setup_wizard.rs`

**Tests:**
1. `extension_steps_excluded_for_disabled` — registry에 blog 확장이 있어도 enabled=false면 `extension_steps`에 없음
2. `extension_steps_included_for_enabled` — enabled=true면 노출
3. `external_keys_dedup_by_id` — 두 확장이 같은 id의 키를 노출하면 마지막 것 우선 (또는 첫 것 — 선택 명시)
4. `seed_sample_data_called_only_for_active` — `setup_complete` 핸들러가 활성 확장에만 호출
5. `extension_step_handler_routes_to_correct_extension` — `/setup/extension-step/profile` POST가 `ProfileExtension::SetupStep.save_handler.save`를 호출
6. `unknown_step_returns_404` — 등록되지 않은 step_id는 404

**Acceptance:**
- `cargo test -p oxipage-core --test setup_wizard` 6개 모두 통과

---

## Phase 3 — OpenAPI 동적 생성

목표: `openapi.rs`가 registry를 받아 확장 경로를 자동 조립.

### Task P3.1: openapi_spec 시그니처 변경

**Files:**
- Modify: `crates/oxipage-core/src/openapi.rs`
- Modify: `crates/oxipage-core/src/http.rs` (또는 build_app 호출자)
- Modify: `crates/oxipage-core/src/lib.rs` (외부에서 호출되는 곳)

**Changes:**
1. `openapi_spec(base_url: &str)` → `openapi_spec(base_url: &str, registry: &ExtensionRegistry)`.
2. 확장 경로 자동 조립:
   ```rust
   fn extension_paths(ext: &Arc<dyn Extension>) -> Value {
       let id = ext.id();
       let name = ext.display_name(crate::extension::Lang::En);
       json!({
           format!("/api/console/{id}/"): {
               "get": {"tags": [id], "summary": format!("{name} list"),
                       "security": [], "responses": ok_data("[]")},
               "post": {"tags": [id], "summary": format!("{name} create (post:write)"),
                        "security": [{"bearer": []}], "responses": ok_data("{}")}
           },
           format!("/api/console/{id}/{{slug}}"): {
               "get": {"tags": [id], "summary": "show", "security": [], "responses": ok_data("{}")},
               "patch": {"tags": [id], "summary": "update (post:write)", "security": [{"bearer": []}]},
               "delete": {"tags": [id], "summary": "delete", "security": [{"bearer": []}]}
           },
           format!("/api/console/{id}/{{slug}}/publish"): {
               "post": {"tags": [id], "summary": "publish (post:publish)",
                        "security": [{"bearer": []}]}
           }
       })
   }
   ```
3. 기존 paths에서 수동 등록된 9개 확장 항목 (62-99 라인) 전부 제거. registry.iter() 순회로 대체.
4. `http.rs::build_app`이 `state`를 받으므로 `openapi_spec("", &state.registry)` 호출. `setup_status_handler`가 이미 `state.registry`를 받으니 영향 없음.
5. 테스트 헬퍼: registry 의존성 없는 `openapi_spec_static()`은 제거하고 테스트는 작은 mock registry 사용.

**Acceptance:**
- `cargo test --workspace` 163 + 6 = 169 모두 통과
- `cargo clippy --workspace --all-targets -- -D warnings` 깨끗
- `/api/console/docs/openapi.json`에 9개 확장 경로가 자동 등장 (smoke test)

---

## Phase 4 — 확장에 hook 구현

목표: profile/blog/movies/books/activity 확장에 새 hook 구현. 코어가 더 이상 도메인 SQL/데이터를 알지 못함.

### Task P4.1: ProfileExtension::setup_wizard_step

**Files:**
- Modify: `crates/oxipage-ext-profile/src/lib.rs`

**Changes:**
1. `SetupStep` 구조체 생성: id="profile", title_ko="프로필", title_en="Profile", description_ko="사이트에 표시할 신상 정보", description_en="Profile info displayed on your site"
2. `SetupField[]`:
   - `display_name` Text required
   - `tagline_ko` Text optional
   - `tagline_en` Text optional
   - `github_username` Text optional
   - `bio_ko` Textarea optional
   - `bio_en` Textarea optional
3. `SetupSaveHandler` impl — `save`가 form에서 각 필드 추출해 `repo::update` 호출. 단, 기존 profile `repo.rs::update`가 `Option<String>` 필드 묶음을 받음. 새 wrapper `update_from_setup_form(pool, form)`을 repo.rs에 추가하거나 inline SQL.
4. `Extension::setup_wizard_step`이 Some(step) 반환.

**Acceptance:**
- `cargo build -p oxipage-ext-profile` 성공
- `cargo test -p oxipage-ext-profile` 기존 테스트 통과

### Task P4.2: BlogExtension::seed_sample_data

**Files:**
- Modify: `crates/oxipage-ext-blog/src/lib.rs`
- Modify: `crates/oxipage-ext-blog/src/repo.rs` (필요 시)

**Changes:**
1. `repo.rs`에 `seed_welcome_post(pool: &SqlitePool) -> anyhow::Result<()>` 추가 — SAMPLE_POST 데이터와 동일한 INSERT (slug="환영합니다", title="환영합니다", body=markdown) 단, 블로그 repo가 작성. body는 blog_extension에 `const WELCOME_POST_BODY: &str = ...`로 두거나 별도 `content/welcome.md` include_str.
2. `Extension::seed_sample_data`가 그 함수 호출.
3. 기존 `oxipage-core/src/setup.rs`의 `SAMPLE_POST_*` 상수/INSERT SQL 전부 제거.

**Acceptance:**
- `cargo build -p oxipage-ext-blog` 성공
- `cargo test -p oxipage-ext-blog` 기존 테스트 통과

### Task P4.3: MoviesExtension::external_api_keys

**Files:**
- Modify: `crates/oxipage-ext-movies/src/lib.rs`

**Changes:**
1. `external_api_keys()`가 `vec![ExternalApiKey { id: "tmdb_key", label_ko: "TMDB API 키", label_en: "TMDB API key", env_var: "OXIPAGE_TMDB_KEY", required: false, scope: ExtensionConfig }]` 반환.
2. 기본 `save_external_key` impl이 작동 (env + extension_state.config). 추가로 `tmdb_api_key_env` 환경변수가 set 되어 있으면 그쪽을 우선 (기존 `IntegrationsConfig::tmdb_key()` 동작). 기본 impl은 env_var에 set한 이름을 config JSON에 저장하지만, movies 확장은 이 키가 이미 `IntegrationsConfig`에 등록되어 있음을 활용할 수도 있음. v1에서는 기본 impl 그대로 사용.

**Acceptance:**
- `cargo build -p oxipage-ext-movies` 성공

### Task P4.4: BooksExtension::external_api_keys

**Files:**
- Modify: `crates/oxipage-ext-books/src/lib.rs`

**Changes:**
1. `external_api_keys()`가 `vec![ExternalApiKey { id: "aladin_key", label_ko: "알라딘 TTBKey", label_en: "Aladin TTBKey", env_var: "OXIPAGE_ALADIN_TTBKEY", required: false, scope: ExtensionConfig }]` 반환.

**Acceptance:**
- `cargo build -p oxipage-ext-books` 성공

### Task P4.5: ActivityExtension::external_api_keys

**Files:**
- Modify: `crates/oxipage-ext-activity/src/lib.rs`

**Changes:**
1. `external_api_keys()`가 `vec![ExternalApiKey { id: "github_username", label_ko: "GitHub 사용자명", label_en: "GitHub username", env_var: "OXIPAGE_GITHUB_USERNAME", required: false, scope: EnvOnly }]` 반환.
2. `EnvOnly` scope이라 env에만 set, config JSON에는 저장 안 함.

**Acceptance:**
- `cargo build -p oxipage-ext-activity` 성공

---

## Phase 5 — Web: 위저드 동적 step 조립

목표: admin-web 위저드가 status 응답으로 step을 동적 조립. StepProfile/StepContent 제거. GenericStep + ExternalKeysStep 신규.

### Task P5.1: 신규 GenericStep + ExternalKeysStep

**Files:**
- Create: `web/src/setup/GenericStep.tsx`
- Create: `web/src/setup/ExternalKeysStep.tsx`

**GenericStep.tsx:**
- Props: `step: ExtensionStepInfo`, `onNext(form)`, `onBack`, `loading`
- `step.fields` 순회해 `Text → <Input>`, `Textarea → <Textarea>`, `Url → <Input type="url">` 렌더
- "건너뛰기" 버튼 (모든 필드가 optional이면) 또는 required 필드가 비면 비활성화
- Submit 시 form JSON object 만들어 `onNext(form)`

**ExternalKeysStep.tsx:**
- Props: `keys: ExternalApiKey[]`, `onNext(values)`, `onBack`, `loading`
- 각 키마다 input (required면 *, optional이면 "(선택)")
- "건너뛰기" 또는 "저장" 버튼
- Submit 시 `{ values: { id: value, ... } }` 객체 만들어 `onNext`

**Acceptance:**
- `cd web && bun run build` 성공

### Task P5.2: SetupWizard.tsx 동적 조립

**Files:**
- Modify: `web/src/setup/SetupWizard.tsx`

**Changes:**
1. switch-case 제거. status 응답으로 step 목록 빌드:
   ```ts
   const steps = useMemo(() => {
     if (!status) return [];
     const base = [
       { id: "site", type: "site", label: "사이트" },
       { id: "extensions", type: "extensions", label: "확장 선택" },
     ];
     const extSteps = status.extension_steps.map(s => ({
       id: s.id, type: "extension-step", label: s.title_ko, step: s,
     }));
     const extKeys = status.external_api_keys.length > 0
       ? [{ id: "external-keys", type: "external-keys", label: "외부 API 키" }]
       : [];
     return [
       ...base,
       ...extSteps,
       ...extKeys,
       { id: "theme", type: "theme", label: "테마 & 레이아웃" },
       { id: "done", type: "done", label: "완료" },
     ];
   }, [status]);
   ```
2. `currentStep = steps[step]`, step.type별 분기:
   - `site` → StepSite
   - `extensions` → StepExtensions
   - `extension-step` → GenericStep
   - `external-keys` → ExternalKeysStep
   - `theme` → StepTheme
   - `done` → StepDone
3. 각 step의 onNext가 알맞은 API 호출. extension-step은 `submitExtensionStep(id, form)`, external-keys는 `submitExternalKeys(values)`.
4. `StepIndicator`가 `steps.length` 사용.

**Acceptance:**
- `cd web && bun run build` 성공

### Task P5.3: api.ts + StepProfile/StepContent 삭제

**Files:**
- Modify: `web/src/setup/api.ts`
- Delete: `web/src/setup/StepProfile.tsx`
- Delete: `web/src/setup/StepContent.tsx`

**api.ts:**
- `submitExtensionStep(stepId: string, form: Record<string, string>)` 추가
- `submitExternalKeys(values: Record<string, string>)` 추가
- `submitProfile`, `submitContent` 제거

**Acceptance:**
- `cd web && bun run build` 성공 (TS 컴파일 깨끗)

---

## Phase 6 — 문서

### Task P6.1: extension-sdk.md에 hook 문서화

**Files:**
- Modify: `docs/extension-sdk.md`

**Changes:**
- § 3 "Core rules" 다음에 새 섹션 추가:
  - "Setup wizard hooks": 4개 hook (setup_wizard_step, external_api_keys, seed_sample_data, save_external_key) 사용법, default behavior, custom save handler 패턴.
  - 작은 예제 (profile, blog, movies 각 1-2 snippet).
- "4. Server registration"은 그대로.

### Task P6.2: doc/13 갱신

**Files:**
- Modify: `doc/13-first-run-ux.md`

**Changes:**
- §13.5.2 API 표 갱신:
  - 제거: `/setup/profile`, `/setup/content`
  - 추가: `/setup/extension-step/{id}`, `/setup/external-keys`
- §13.7.2 step 조립이 registry 응답 기반으로 동적임을 명시
- §13.5.2 `GET /setup/status` 응답에 `extension_steps`, `external_api_keys` 추가

---

## Phase 7 — 검증

### Task P7.1: cargo test + clippy

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```
- 169+ tests 전부 통과
- clippy 깨끗

### Task P7.2: web build

```bash
cd web && bun run build
```
- TS/Vite 빌드 성공, dist 생성

### Task P7.3: 수동 smoke

```bash
# 1) blog/profile만 enabled 상태로 setup wizard 렌더링 확인
# 2) movies/books만 enabled 상태로 setup wizard 렌더링 확인
# 3) blog/blog/profile 모두 disabled 상태로 setup wizard (모든 extension step 사라짐)
# 4) /api/console/docs/openapi.json에 9개 확장 경로 자동 등장
```

각 시나리오의 검증 방법:
- StepProfile.tsx가 삭제되어 컴파일 시점에 사라졌는지 확인 (`grep -r StepProfile web/src` → 0 hits)
- extension_id="unknown_extension"인 step에 POST 시 404 반환
- setup wizard가 enabled된 확장의 step만 그림 (status 응답 확인)

**Acceptance:**
- 모든 smoke 시나리오 통과
- 회귀: 기존 163개 테스트 + 신규 6개 테스트 + 신규 step 조립 정상 동작

---

## 작업 의존성 그래프

```
P1 (Core trait) ──▶ P2 (setup 핸들러) ──▶ P4 (확장 hook)
                  ├▶ P3 (openapi)
                  └▶ P5 (web wizard) ◀── P2 응답 변경
P4 ──▶ P2.2 테스트 (extension_steps 노출)
P3 ──▶ P6.1 extension-sdk 문서
P5 ──▶ P6.2 doc/13 문서
모두 ──▶ P7 검증
```

순차 의존성 외에는 병렬 가능. 단, Phase 5(web)는 Phase 2 응답 변경을 알아야 하므로 P2 → P5.

---

## 위험 / 주의

1. **Extension trait 변경 = 컴파일 시 모든 9개 확장 깨짐 가능.** default impl로 안전망 제공하면 영향 없음. P1 끝나면 `cargo build --workspace`로 즉시 확인.
2. **`set_extension_config` 위치.** trait default impl에서 `setup::set_extension_config` 호출하려면 `oxipage_core::setup` 모듈이 `extension.rs`보다 먼저 정의됨. 현재 setup.rs가 extension.rs를 import하므로 cycle 가능성. 해결: trait default impl이 `set_extension_config` 호출 안 하고 in-place SQL 작성 (env_var set + extension_state.config UPDATE). 또는 `set_extension_config`을 별도 모듈(`config.rs` 등)로 이동.
   - **결정**: `set_extension_config`을 `crate::config` 또는 별도 `crate::extension_config` 모듈로 이동. setup.rs에서 re-export.
3. **ExtensionStepInfo와 SetupStep의 중복.** SetupStep이 `save_handler: Arc<dyn SetupSaveHandler>`를 가지지만 ExtensionStepInfo(클라이언트로 보낼 JSON)는 save_handler를 빼야 함. SetupStep의 fields 부분만 직렬화하도록 별도 타입 필요. 본 계획 §3.3에 반영.
4. **OpenAPI 동적 생성 후 응답 형식 미세 변경.** 테스트가 정확한 summary 텍스트를 단언하면 깨짐. 현재는 summary 자유도가 낮으니 신규는 `format!("{name} list")` 같은 일반 패턴으로. 기존 테스트가 summary를 단언하는지 확인 필요.
5. **`ActivityExtension::external_api_keys`가 `github_username`을 노출하는 게 맞나?** activity 확장의 `GithubClient::with_username(ctx.config.integrations.github_username())` — 이건 config에서 읽음. setup wizard에서 키 받으면 그 값을 env var에 set하고 config에도 저장. 기존 동작과 호환. OK.

---

## 완료 정의

위 7개 phase 모두 완료 + P7.3 smoke 전 시나리오 통과 + `git diff`로 핵심 변경(extension.rs/setup.rs/openapi.rs/web wizard) 모두 의도대로 적용됨 확인.
