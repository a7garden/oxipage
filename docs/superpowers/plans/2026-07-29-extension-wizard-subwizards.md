# 확장 소유 조건부 서브-위자드 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 각 확장이 다중 step 서브-위자드를 소유하고, 선언적 가시성 규칙으로 step을 조건부 표시하며, 외부 API 키를 자기 위자드의 필드로 흡수하도록 setup 마법사를 재설계한다.

**Architecture:** `Extension::setup_wizard_step() -> Option<SetupStep>`(단수)를 `setup_wizard() -> Option<ExtensionWizard { steps: Vec<SetupStep> }>`로 전환한다. 각 step은 `visible_when: Option<VisibilityRule>`을 갖고, **클라이언트**가 직전 step이 반환한 `StepOutcome`으로 가시성을 평가한다. 공유 `external-keys` step을 폐지하고 키를 `SetupFieldKind::Secret` 필드로 흡수한다. 3-phase로 분리, 각 phase는 독립 빌드·테스트·커밋.

**Tech Stack:** Rust (axum 0.8, sqlx, async-trait), TypeScript/React (Vite), SQLite.

**Spec:** `docs/superpowers/specs/2026-07-29-extension-wizard-subwizards-design.md`

**확정된 결정:** 키 흡수(공유 external-keys 폐지) ✓, v1 action step 포함 ✓.

## Global Constraints

- axum 0.8: 경로 파라미터는 `{slug}` 형식 (`:slug` 아님). trailing slash 금지.
- `order`는 SQL 예약어 — 항상 `display_order` 사용.
- setup API는 loopback-only (`setup_gate`, `setup.rs:226`); 완료 후 410.
- 코어는 확장 도메인 컬럼/키 이름을 모른다 — 모든 도메인 SQL/저장은 확장 트레이트 안에서.
- 모든 setup 핸들러 응답은 `DataEnvelope<T>`.
- 위자드 프론트는 `web/` (공개 SPA). 빌드: `cd web && bun run build`. (admin-web 아님.)
- 테스트: `cargo test -p oxipage-core`.
- step id, extension id는 레지스트리 전역 유일.
- 구현은 별도 브랜치에서 (사용자 되돌림 가능성). 각 task 끝에 커밋.

---

## File Structure

**Rust core (`crates/oxipage-core/src/`):**
- `extension.rs` — `Extension` 트레이트, `ExtensionWizard`, `SetupStep`(+`visible_when`), `SetupField`, `SetupFieldKind`(+`Secret`), `SetupSaveHandler`(→`StepOutcome` 반환), `StepOutcome`, `VisibilityRule`, `PrefillSource`, `ExtensionStepInfo`/`ExtensionWizardInfo`. `external_api_keys`/`save_external_key`/`ExternalApiKey`/`ExternalKeyScope` 폐지(Phase 2).
- `setup.rs` — `StatusResult.extension_wizards`, `setup_status_handler`, `setup_extension_step_handler`(네임스페이스 + `StepOutcome` 반환), `/setup/external-keys` 제거(Phase 2).
- `tests/setup_wizard.rs` — 시그니처 갱신 + 다중 step/가시성/action 테스트.

**확장들:** profile(`setup_wizard`); movies/books/activity(Phase 2: 키 흡수; Phase 3: action step).

**프론트 (`web/src/setup/`):** `api.ts`, `SetupWizard.tsx`, `GenericStep.tsx`, 신규 `ExtensionSubWizard.tsx`/`visibility.ts`, 삭제 `ExternalKeysStep.tsx`(Phase 2).

---

## Phase 1 — Foundation (단수 → 복수 위자드, 그룹화)

> 의존성 없음. 키 메커니즘·action step 모두 건드리지 않음. 이 phase만으로 빌드·테스트·커밋 가능.

### Task 1: 코어 — `setup_wizard()` 복수 API + status 그룹화 + 네임스페이스 라우트

**Files:**
- Modify: `crates/oxipage-core/src/extension.rs`, `crates/oxipage-core/src/setup.rs`, `crates/oxipage-ext-profile/src/lib.rs`
- Test: `crates/oxipage-core/tests/setup_wizard.rs`

**Interfaces:**
- Produces: `pub fn setup_wizard(&self) -> Option<ExtensionWizard>` (기본 `None`); `pub struct ExtensionWizard { pub steps: Vec<SetupStep> }`; `pub struct ExtensionWizardInfo { extension_id, display_name, steps: Vec<ExtensionStepInfo> }`; `StatusResult.extension_wizards: Vec<ExtensionWizardInfo>`; 라우트 `POST /setup/extension-step/{ext_id}/{step_id}`.
- Consumes: 기존 `SetupStep`/`SetupField`/`SetupSaveHandler`/`PrefillSource` (변경 없음). `VisibilityRule` enum은 이 phase에서 타입만 정의(항상 `None`) — Phase 3에서 실사용.

- [ ] **Step 1: 실패 테스트 작성 — 다중 step 직렬화**

`tests/setup_wizard.rs`에 두 step을 반환하는 헬퍼 확장 추가:

```rust
struct MultiStepExt;
#[async_trait]
impl Extension for MultiStepExt {
    fn id(&self) -> &'static str { "multi" }
    fn display_name(&self, l: Lang) -> String { "Multi".into() }
    fn migrations(&self) -> Vec<Migration> { vec![] }
    fn routes(&self) -> Router<AppState> { Router::new() }
    fn setup_wizard(&self) -> Option<ExtensionWizard> {
        Some(ExtensionWizard {
            steps: vec![
                SetupStep { id: "multi_a", title_ko: "A", title_en: "A",
                    description_ko: "", description_en: "", fields: vec![],
                    save_handler: Arc::new(NoopSave),
                    prefill: BTreeMap::new(), visible_when: None },
                SetupStep { id: "multi_b", title_ko: "B", title_en: "B",
                    description_ko: "", description_en: "", fields: vec![],
                    save_handler: Arc::new(NoopSave),
                    prefill: BTreeMap::new(), visible_when: None },
            ],
        })
    }
}

#[tokio::test]
async fn status_returns_multiple_steps_per_wizard() {
    let app = build_app(vec![Arc::new(MultiStepExt)]).await;
    let json: serde_json::Value = body_json(get_resp(app, "/api/console/setup/status").await);
    let wizards = json["data"]["extension_wizards"].as_array().unwrap();
    assert_eq!(wizards.len(), 1);
    let steps = wizards[0]["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 2);
    assert_eq!(steps[0]["id"], "multi_a");
    assert_eq!(steps[1]["id"], "multi_b");
}
```
(`NoopSave`/`body_json`/`get_resp`/`build_app` 헬퍼는 기존 파일 패턴에 맞춰 추가/재사용. `NoopSave::save`는 이 phase에선 `Ok(())` — `StepOutcome` 반환은 Phase 3.)

- [ ] **Step 2: 테스트 실패 확인**

Run: `cargo test -p oxipage-core --test setup_wizard status_returns_multiple_steps_per_wizard`
Expected: 컴파일 에러 (`setup_wizard`/`ExtensionWizard` 미정의).

- [ ] **Step 3: 코어 타입 구현 (`extension.rs`)**

- `VisibilityRule` enum 정의: `FieldNotEmpty { step_id, field }`, `FieldEquals { step_id, field, value }`, `All(Vec)`, `Any(Vec)`. `#[derive(Debug, Clone, serde::Serialize)]` + `#[serde(tag="kind", rename_all="snake_case")]`. (이 phase에선 사용 안 함.)
- `ExtensionWizard { pub steps: Vec<SetupStep> }`.
- `SetupStep`에 `pub visible_when: Option<VisibilityRule>` 필드 추가.
- `ExtensionStepInfo`에 `pub visible_when: Option<VisibilityRule>`, `pub is_action: bool` 필드 추가; `from_step`에서 채움 (`is_action: step.fields.is_empty()`).
- `ExtensionWizardInfo { pub extension_id: String, pub display_name: ExtDisplayName, pub steps: Vec<ExtensionStepInfo> }`.
- 트레이트: `fn setup_wizard_step(&self) -> Option<SetupStep>` → `fn setup_wizard(&self) -> Option<ExtensionWizard>` (기본 `None`). **이 phase에선 `external_api_keys`/`save_external_key` 유지** (Phase 2 폐지).

- [ ] **Step 4: status 핸들러 + StatusResult (`setup.rs`)**

- `StatusResult`: `extension_steps: Vec<ExtensionStepInfo>` → `extension_wizards: Vec<ExtensionWizardInfo>` (`external_api_keys` 필드는 Phase 2까지 유지).
- `setup_status_handler`: 활성 확장의 `setup_wizard()`를 호출해 `ExtensionWizardInfo { extension_id: ext.id(), display_name, steps }` 조립 (각 step은 `ExtensionStepInfo::from_step`). 비활성 확장은 제외 (기존 `is_active` 게이트 유지).

- [ ] **Step 5: 네임스페이스 라우트 (`setup.rs`)**

- `setup_routes`: `/setup/extension-step/{step_id}` → `/setup/extension-step/{ext_id}/{step_id}`.
- `setup_extension_step_handler`: `Path<(String, String)>` (ext_id, step_id). `ext_id`로 활성 확장 찾기 → 그 `setup_wizard().steps`에서 `step.id == step_id` 매칭 → 없으면 404 → `step.save_handler.save(...)`. 응답은 이 phase에선 기존 `SimpleOk` (`StepOutcome` 반환은 Phase 3).

- [ ] **Step 6: profile 마이그레이션 (`crates/oxipage-ext-profile/src/lib.rs`)**

`fn setup_wizard_step()` → `fn setup_wizard() -> Option<ExtensionWizard>`. 기존 `SetupStep`을 `ExtensionWizard { steps: vec![기존 step에 visible_when: None 추가] }`로 감쌈.

- [ ] **Step 7: 기존 테스트 갱신 + 통과**

`tests/setup_wizard.rs`의 `DemoExt`/`setup_status_includes_extension_steps`/`disabled_extension_excluded_from_status`를 `setup_wizard()` 시그니처로 갱신. (`external_api_keys` 관련 테스트는 Phase 2까지 유지 — status 응답에 `external_api_keys` 필드가 아직 있으므로.)
Run: `cargo test -p oxipage-core --test setup_wizard`
Expected: PASS.

- [ ] **Step 8: 컴파일 + 커밋**

Run: `cargo build -p oxipage-core -p oxipage-ext-profile`
```bash
git add -A && git commit -m "feat(setup): 단수 setup_wizard_step → 복수 setup_wizard + 네임스페이스 라우트"
```

---

### Task 2: 프론트 — 위자드 그룹화 렌더 (조건부 없이)

**Files:**
- Modify: `web/src/setup/api.ts`, `web/src/setup/SetupWizard.tsx`

**Interfaces:**
- Consumes: Task 1의 `extension_wizards: ExtensionWizardInfo[]`.
- Produces: 전역 시퀀스에 확장당 한 엔트리; steps 평탄화 + 확장 display_name 헤더 (Phase 3에서 `ExtensionSubWizard`로 승격).

- [ ] **Step 1: api.ts 타입 갱신**

`ExtensionStepInfo`에 `visible_when?`, `is_action` 추가 (이 phase에선 미사용). `ExtensionWizardInfo { extension_id, display_name, steps }` 추가. `SetupStatus.extension_wizards` 필드로 교체 (`extension_steps` 제거). `submitExtensionStep(extId, stepId, form)` 시그니처 + 경로 `/extension-step/${extId}/${stepId}`.

- [ ] **Step 2: buildSteps 그룹화 (`SetupWizard.tsx`)**

`buildSteps`가 각 wizard의 steps를 `{ type: "extension-step", step, extensionId }`로 평탄화해 전역 시퀀스에 넣되, wizard 경계마다 display_name 헤더 엔트리를 앞에 붙임. (Phase 3에서 서브-위자드로 교체되므로 여기선 최소 평탄화 + 헤더.)

- [ ] **Step 3: 빌드 확인**

Run: `cd web && bun run build`
Expected: 성공.

- [ ] **Step 4: 커밋**

```bash
git add -A && git commit -m "feat(setup-web): 위자드 그룹화 렌더 (평탄화 + 확장 헤더)"
```

---

## Phase 2 — 키 흡수 (Secret 필드, 공유 external-keys 폐지)

> 주의: Task 3이 트레이트 메서드를 제거해 movies/books/activity를 동시에 깨뜨린다. **Task 3→4를 같은 커밋 시퀀스** 안에서 끝내야 workspace가 녹색.

### Task 3: 코어 — `Secret` 필드 종류 + external-keys 메커니즘 폐지

**Files:**
- Modify: `crates/oxipage-core/src/extension.rs`, `crates/oxipage-core/src/setup.rs`
- Test: `crates/oxipage-core/tests/setup_wizard.rs`

**Interfaces:**
- Produces: `SetupFieldKind::Secret`. 제거: `Extension::external_api_keys()`, `Extension::save_external_key()`, `ExternalApiKey`, `ExternalKeyScope`, `StatusResult.external_api_keys`, `POST /setup/external-keys` 라우트 + `setup_external_keys_handler` + `ExternalKeysInput`.
- Consumes: 기존 `persist_extension_config` (`extension.rs:374`) — 이제 각 확장의 키-step save_handler가 직접 호출.

- [ ] **Step 1: 실패 테스트 작성 — external_api_keys 제거 검증**

기존 `setup_status_includes_external_api_keys` / `KeyOnlyExt` 테스트를 삭제. 대신:

```rust
#[tokio::test]
async fn status_has_no_external_api_keys_field() {
    // KeyOnlyExt는 이제 setup_wizard()로 단일 Secret-field step 노출 (Task 4 패턴 템플릿)
    let app = build_app(vec![Arc::new(KeyOnlyExt)]).await;
    let json: serde_json::Value = body_json(get_resp(app, "/api/console/setup/status").await);
    assert!(json["data"].get("external_api_keys").is_none());
}
```

- [ ] **Step 2: 테스트 실패 확인**

Run: `cargo test -p oxipage-core --test setup_wizard status_has_no_external_api_keys_field`
Expected: FAIL (필드 아직 존재) 또는 컴파일 에러.

- [ ] **Step 3: SetupFieldKind::Secret 추가 (`extension.rs`)**

`SetupFieldKind`에 `Secret` variant 추가 (`#[serde(tag="type", rename_all="snake_case")]` → `"secret"`).

- [ ] **Step 4: external-keys 메커니즘 제거**

- `extension.rs`: 트레이트에서 `external_api_keys()` / `save_external_key()` 제거. `ExternalApiKey`, `ExternalKeyScope` 제거.
- `setup.rs`: `StatusResult.external_api_keys` 필드 제거, `setup_status_handler`에서 키 수집 로직 제거, `setup_routes`에서 `/setup/external-keys` 제거, `setup_external_keys_handler` + `ExternalKeysInput` 제거.

- [ ] **Step 5: 기존 키 테스트 정리**

`tests/setup_wizard.rs`: `KeyOnlyExt`가 `external_api_keys()` 대신 `setup_wizard()`로 단일 Secret-field step을 반환하도록 변경 (Task 4에서 실제 확장에 적용할 패턴의 템플릿). `submitExternalKeys` 관련 통합 테스트 제거.
Run: `cargo test -p oxipage-core --test setup_wizard`
Expected: PASS (코어 한정; movies/books/activity는 Task 4에서 복구).

- [ ] **Step 6: 코어 빌드 확인**

Run: `cargo build -p oxipage-core`
Expected: 성공. (workspace 전체는 Task 4 후 녹색.)

---

### Task 4: 확장 마이그레이션 + 프론트 Secret 렌더

**Files:**
- Modify: `crates/oxipage-ext-movies/src/lib.rs`, `crates/oxipage-ext-books/src/lib.rs`, `crates/oxipage-ext-activity/src/lib.rs`, `web/src/setup/GenericStep.tsx`, `web/src/setup/api.ts`, `web/src/setup/SetupWizard.tsx`
- Delete: `web/src/setup/ExternalKeysStep.tsx`

**Interfaces:**
- Consumes: Task 3의 `Secret` kind + 폐지된 `external_api_keys`.
- Produces: movies/books/activity 각각 1-step `setup_wizard()`(Secret/text 키 필드 + `persist_extension_config` 저장). 프론트 `GenericStep`의 `Secret` 렌더.

- [ ] **Step 1: movies 키-step 위자드**

`crates/oxipage-ext-movies/src/lib.rs`: `external_api_keys()` 제거. `setup_wizard()` 추가 — 단일 step `movies_key`, 필드 `tmdb_key` (`Secret`, required=false). `MoviesKeySave` 핸들러: `persist_extension_config(ctx, "movies", "tmdb_key", form["tmdb_key"])` + `std::env::set_var("OXIPAGE_TMDB_KEY", val)`. (테스트/가져오기 step은 Phase 3 Task 7.)

- [ ] **Step 2: books 동일 패턴**

`crates/oxipage-ext-books/src/lib.rs`: `aladin_key` → 단일 step `books_key`, `Secret` 필드, `persist_extension_config("books","aladin_key")` + `OXIPAGE_ALADIN_KEY`.

- [ ] **Step 3: activity 동일 패턴**

`crates/oxipage-ext-activity/src/lib.rs`: `github_username` → 단일 step `activity_github`, 필드 `github_username` (`Text` — 공개 식별자라 Secret 아님). `persist_extension_config("activity","github_username")`.

- [ ] **Step 4: workspace 빌드 + 테스트 녹색**

Run: `cargo build --workspace && cargo test --workspace`
Expected: PASS (Task 3의 트레이트 제거로 깨졌던 세 확장 복구).

- [ ] **Step 5: 프론트 GenericStep Secret 렌더**

`GenericStep.tsx`: `f.type === "secret"` 분기 → `<Input type="password" autocomplete="off">`. `initialValues`에서 secret 필드는 빈 문자열 강제 (프리필 금지).

- [ ] **Step 6: ExternalKeysStep 삭제 + 잔여 정리**

`ExternalKeysStep.tsx` 삭제. `api.ts`: `submitExternalKeys`, `ExternalApiKey` 타입 제거. `SetupWizard.tsx`: `external-keys` step 타입/케이스 제거.

- [ ] **Step 7: 프론트 빌드 + 커밋**

Run: `cd web && bun run build`
```bash
git add -A && git commit -m "feat(setup): movies/books/activity 키를 각자 Secret-field 위자드로 흡수"
```

---

## Phase 3 — 조건부 가시성 + action step

### Task 5: 코어 — `VisibilityRule` 평가 + `StepOutcome` 반환

**Files:**
- Modify: `crates/oxipage-core/src/extension.rs`, `crates/oxipage-core/src/setup.rs`, `crates/oxipage-ext-profile/src/lib.rs`, `crates/oxipage-ext-movies/src/lib.rs`, `crates/oxipage-ext-books/src/lib.rs`, `crates/oxipage-ext-activity/src/lib.rs`
- Test: `crates/oxipage-core/tests/setup_wizard.rs`

**Interfaces:**
- Produces: `SetupSaveHandler::save(...) -> anyhow::Result<StepOutcome>`; `StepOutcome { values: Map<String,Value> }` + `from_form`; `setup_extension_step_handler`가 `Json<DataEnvelope<StepOutcome>>` 반환; `ExtensionStepInfo.visible_when` 실직렬화.
- Consumes: Task 1의 `VisibilityRule` 타입 정의, Task 3의 `Secret`.

- [ ] **Step 1: 실패 테스트 — StepOutcome 반환 + 가시성 직렬화**

```rust
#[tokio::test]
async fn extension_step_returns_outcome() {
    let app = build_app(vec![Arc::new(OutcomeExt)]).await;
    let json: serde_json::Value = body_json(post_resp(app,
        "/api/console/setup/extension-step/outcome/step_one", r#"{"name":"hi"}"#).await);
    assert_eq!(json["data"]["values"]["name"], "hi");
}

#[tokio::test]
async fn status_serializes_visible_when() {
    // MultiStepExt 의 두 번째 step 에 visible_when: FieldNotEmpty{step_id:"multi_a", field:"x"}
    let json: serde_json::Value = body_json(get_resp(build_app(vec![Arc::new(MultiStepExt)]).await,
        "/api/console/setup/status").await);
    let rule = &json["data"]["extension_wizards"][0]["steps"][1]["visible_when"];
    assert_eq!(rule["kind"], "field_not_empty");
    assert_eq!(rule["step_id"], "multi_a");
    assert_eq!(rule["field"], "x");
}
```

- [ ] **Step 2: 테스트 실패 확인**

Run: `cargo test -p oxipage-core --test setup_wizard extension_step_returns_outcome status_serializes_visible_when`
Expected: FAIL.

- [ ] **Step 3: SetupSaveHandler 반환형 변경 (`extension.rs`)**

`save(...) -> anyhow::Result<StepOutcome>`. `StepOutcome { values: serde_json::Map<String, serde_json::Value> }` + `Default` + `from_form(form)`.

- [ ] **Step 4: 모든 save 핸들러 갱신**

- profile (`crates/oxipage-ext-profile/src/lib.rs`): 폼 저장 후 `Ok(StepOutcome::from_form(form))`.
- movies/books/activity 키-step 핸들러: 저장 후 `Ok(StepOutcome::from_form(form))`.
- `tests/setup_wizard.rs`의 `NoopSave`: `Ok(StepOutcome::default())`.

- [ ] **Step 5: 핸들러가 StepOutcome 반환 (`setup.rs`)**

`setup_extension_step_handler`가 `save_handler.save(...)` 결과를 `Json<DataEnvelope<StepOutcome>>`로 반환 (기존 `SimpleOk` 대체). `ExtensionStepInfo` 직렬화에 `visible_when` 포함 — Phase 1에서 필드만 만들어둔 것을 실제로 채움.

- [ ] **Step 6: 통과 + 커밋**

Run: `cargo test --workspace`
Expected: PASS.
```bash
git add -A && git commit -m "feat(setup): save 핸들러 StepOutcome 반환 + visible_when 직렬화"
```

---

### Task 6: 프론트 — `ExtensionSubWizard` + `evalRule`

**Files:**
- Create: `web/src/setup/visibility.ts`, `web/src/setup/ExtensionSubWizard.tsx`
- Modify: `web/src/setup/GenericStep.tsx`, `web/src/setup/SetupWizard.tsx`, `web/src/setup/api.ts`

**Interfaces:**
- Consumes: Task 5의 `StepOutcome` 반환, `visible_when` 직렬.
- Produces: `evalRule(rule, outcomes)` 순수 함수; `<ExtensionSubWizard wizard onSubmitStep onComplete onExitBack>`; action step 렌더.

- [ ] **Step 1: evalRule 단위 테스트 작성**

`web/src/setup/visibility.test.ts`:

```ts
import { evalRule } from "./visibility";
const mk = (m: Record<string, Record<string,string>>) =>
  new Map(Object.entries(m).map(([k,v]) => [k, new Map(Object.entries(v))]));

test("field_not_empty", () => {
  expect(evalRule({kind:"field_not_empty", step_id:"s", field:"f"}, mk({}))).toBe(false);
  expect(evalRule({kind:"field_not_empty", step_id:"s", field:"f"}, mk({s:{f:"x"}}))).toBe(true);
});
test("field_equals", () => {
  const r = {kind:"field_equals", step_id:"s", field:"f", value:"true"} as const;
  expect(evalRule(r, mk({s:{f:"true"}}))).toBe(true);
  expect(evalRule(r, mk({s:{f:"no"}}))).toBe(false);
});
test("all / any", () => {
  const t = {kind:"field_not_empty", step_id:"s", field:"a"} as const;
  const f = {kind:"field_not_empty", step_id:"s", field:"b"} as const;
  expect(evalRule({kind:"all", all:[t,f]}, mk({s:{a:"1"}}))).toBe(false);
  expect(evalRule({kind:"any", any:[t,f]}, mk({s:{a:"1"}}))).toBe(true);
});
```

- [ ] **Step 2: evalRule 구현 (`visibility.ts`)**

명세 §6.2 의 `evalRule` 구현. `get = (sid,f) => o.get(sid)?.get(f) ?? ""`.

- [ ] **Step 3: ExtensionSubWizard 컴포넌트 (`ExtensionSubWizard.tsx`)**

내부 `stepIdx` + `outcomes: Map<stepId, Map<field,string>>` + `error`. 루프: 현재 step의 `visible_when`을 `evalRule`로 평가 → 거짓이면 `stepIdx++` 반복 → visible step 렌더 (`is_action`이면 action 버튼, 아니면 `GenericStep`). 제출 → `onSubmitStep(step.id, form)` → `outcomes[step.id] = result.values` 머지 → `stepIdx++`. 마지막 visible step 이후 `onComplete()`. 첫 visible step에서 "← 이전" → `onExitBack()`.

- [ ] **Step 4: GenericStep action 렌더 + SetupWizard 연결**

- `GenericStep.tsx`: `step.fields.length === 0` (또는 `is_action`) → 폼 대신 단일 "실행 →" 버튼 (제목 유도).
- `SetupWizard.tsx`: `buildSteps`를 `[site, extensions, ...wizards.map(w => ({type:"extension-wizard", wizard:w})), theme, done]`으로 변경 (Task 2의 평탄화를 서브-위자드로 교체). `extension-wizard` 케이스에서 `<ExtensionSubWizard>` 렌더, `onComplete` → 전역 `stepIdx+1`, `onExitBack` → `stepIdx-1`.

- [ ] **Step 5: api.ts StepOutcome 반환형**

`submitExtensionStep` 반환형 `Promise<StepOutcome>` (`{ values: Record<string,string> }`).

- [ ] **Step 6: 빌드 + 테스트 + 커밋**

Run: `cd web && bun run build && bun test` (또는 프로젝트 테스트 러너)
```bash
git add -A && git commit -m "feat(setup-web): ExtensionSubWizard + 클라이언트 가시성 평가"
```

---

### Task 7: movies/books/activity action step 추가 + profile outcome

**Files:**
- Modify: `crates/oxipage-ext-movies/src/lib.rs`, `crates/oxipage-ext-books/src/lib.rs`, `crates/oxipage-ext-activity/src/lib.rs`, `crates/oxipage-ext-profile/src/lib.rs`
- Test: `crates/oxipage-core/tests/setup_wizard.rs`

**Interfaces:**
- Consumes: Task 5의 action step (`fields: vec![]`) + `visible_when` + `StepOutcome`.
- Produces: movies/books = `[키 → 테스트(action) → 가져오기(action)]`; activity = `[사용자명 → 동기화(action)]`.

- [ ] **Step 1: movies 테스트/가져오기 step 추가**

`crates/oxipage-ext-movies/src/lib.rs`: 위자드 steps를 3개로 확장:
- `movies_key` (Secret, Task 4)
- `movies_test` — `fields: vec![]`, save_handler가 TMDB 핑 → `Ok(StepOutcome{ values: {"connection_ok":"true"} })` (실패 시 `{"connection_ok":"false","error":...}`). `visible_when: Some(FieldNotEmpty{ step_id:"movies_key", field:"tmdb_key" })`.
- `movies_import` — `fields: vec![]`, save_handler가 인기작 가져오기 → `Ok(StepOutcome{ values: {"imported": count} })`. `visible_when: Some(FieldEquals{ step_id:"movies_test", field:"connection_ok", value:"true" })`.

- [ ] **Step 2: books 동일 (알라딘)**

`crates/oxipage-ext-books/src/lib.rs`: `books_key` / `books_test` / `books_import`. 동일 visible_when 패턴.

- [ ] **Step 3: activity 동기화 step 추가**

`crates/oxipage-ext-activity/src/lib.rs`: `activity_github` (Task 4) + `activity_sync` — `fields: vec![]`, save_handler가 GitHub 활동 동기화. `visible_when: Some(FieldNotEmpty{ step_id:"activity_github", field:"github_username" })`.

- [ ] **Step 4: profile from_form 확인**

`crates/oxipage-ext-profile/src/lib.rs`: save가 `Ok(StepOutcome::from_form(form))` 반환 (Task 5에서 했으면 확인만).

- [ ] **Step 5: 통합 테스트 — action step 디스패치 + outcome**

`tests/setup_wizard.rs`:

```rust
#[tokio::test]
async fn action_step_dispatched_with_empty_form() {
    // ActionExt: 빈 fields step, save → outcome {ok:"1"}
    let app = build_app(vec![Arc::new(ActionExt)]).await;
    let json: serde_json::Value = body_json(post_resp(app,
        "/api/console/setup/extension-step/action/do_it", "{}").await);
    assert_eq!(json["data"]["values"]["ok"], "1");
}
```

Run: `cargo test --workspace`
Expected: PASS.

- [ ] **Step 6: 스모크 + 커밋**

`cargo run -p oxipage-console -- console` 후 브라우저 `/setup`: movies 켜고 → TMDB 키 입력 → 테스트 버튼 → (키 있을 때) 가져오기 버튼 노출 확인. (자동화 어려우면 cargo test + web build 녹색으로 대체.)
```bash
git add -A && git commit -m "feat(setup): movies/books/activity 조건부 action step 추가"
```

---

## Self-Review

**1. Spec coverage:**
- D1 (복수 위자드) → Task 1. ✓
- D2 (visible_when, 클라이언트 평가) → Task 5 (직렬화) + Task 6 (evalRule). ✓
- D3 (StepOutcome 반환) → Task 5. ✓
- D4 (Secret + external-keys 폐지) → Task 3 + Task 4. ✓
- 네임스페이스 라우트 → Task 1 Step 5. ✓
- 프론트 그룹화 → Task 2 → Task 6 (서브-위자드 승격). ✓
- 확장 마이그레이션 (profile/movies/books/activity) → Task 1/4/7. ✓
- blog/projects/links/novels/scraps → 변경 없음 (명세 §7), task 불필요. ✓

**2. Placeholder scan:** TODO/TBD 없음. 모든 코드 단계에 실제 시그니처/테스트 코드. (수동 스모크는 자동화 대체 허용 명시.)

**3. Type consistency:**
- `setup_wizard()` 시그니처 모든 task에서 일관. ✓
- `StepOutcome` 반환 — Task 5에서 save 반환형 변경 후 Task 6/7 소비; Phase 1의 `NoopSave::Ok(())` → Task 5 `Ok(StepOutcome::default())` 로 이어짐. ✓
- step id (`movies_key`/`movies_test`/`movies_import`, `books_*`, `activity_github`/`activity_sync`) — 명세 §8 시퀀스와 일치. ✓
- `visible_when` 참조 (`step_id`/`field`) — Task 7 규칙이 Task 6 `evalRule` 키와 일치. ✓

**알려진 위험:**
- Task 3이 workspace를 일시적으로 깨뜨림 → Task 3→4 연속 실행 (본문 명시).
- Task 2의 평탄화 렌더는 Task 6에서 서브-위자드로 교체 — 중간 산출물이나 Phase 1을 독립 smoke-testable하게 유지하기 위함.

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-07-29-extension-wizard-subwizards.md`. 구현 시작 시:
1. 별도 브랜치 분기 (`git checkout -b feat/extension-subwizards`) — 사용자 되돌림 가능성.
2. 두 실행 옵션:
   - **Subagent-Driven (권장)** — task별 fresh subagent dispatch + task 간 리뷰, 빠른 이터레이션.
   - **Inline** — executing-plans로 배치 실행 + 체크포인트.
