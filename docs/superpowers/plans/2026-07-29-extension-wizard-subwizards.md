# 확장 소유 조건부 서브-위자드 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 각 확장이 다중 step 서브-위자드를 소유하고, 선언적 가시성 규칙으로 step을 조건부 표시하며, 외부 API 키를 자기 위자드의 필드로 흡수하도록 setup 마법사를 재설계한다.

**Architecture:** `Extension::setup_wizard_step() -> Option<SetupStep>`(단수)를 `setup_wizard() -> Option<ExtensionWizard { steps: Vec<SetupStep> }>`로 전환한다. 각 step은 `visible_when: Option<VisibilityRule>`을 갖고, **클라이언트**가 직전 step이 반환한 `StepOutcome`으로 가시성을 평가한다. 공유 `external-keys` step을 폐지하고 키를 `SetupFieldKind::Secret` 필드로 흡수한다.

**Tech Stack:** Rust (axum 0.8, sqlx, async-trait), TypeScript/React (Vite), SQLite.

**Spec:** `docs/superpowers/specs/2026-07-29-extension-wizard-subwizards-design.md`

> **진행 상태:** **Phase 1은 전체 TDD 디테일로 작성됨** (어떤 미결정 사항에도 의존하지 않음 — 독립적 커밋 가능). **Phase 2/3은 아웃라인만** — 두 미결정 사항(키 흡수 여부, v1 action step 포함 여부)이 확정되면 디테일을 채운다. 사용자 확인 전에 Phase 2/3의 상세 단계를 작성하지 않는다.

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

## File Structure (전체)

**Rust core (`crates/oxipage-core/src/`):**
- `extension.rs` — `Extension` 트레이트, `ExtensionWizard`, `SetupStep`(+`visible_when`), `SetupField`, `SetupFieldKind`(+`Secret`), `SetupSaveHandler`(→`StepOutcome` 반환), `StepOutcome`, `VisibilityRule`, `PrefillSource`, `ExtensionStepInfo`/`ExtensionWizardInfo`. `external_api_keys`/`save_external_key`/`ExternalApiKey`/`ExternalKeyScope` 폐지(Phase 2).
- `setup.rs` — `StatusResult.extension_wizards`, `setup_status_handler`, `setup_extension_step_handler`(네임스페이스 + `StepOutcome` 반환), `/setup/external-keys` 제거(Phase 2).
- `tests/setup_wizard.rs` — 시그니처 갱신 + 다중 step/가시성/action 테스트.

**확장들:** profile(`setup_wizard`), movies/books/activity(Phase 2: 키 흡수; Phase 3: action step).

**프론트 (`web/src/setup/`):** `api.ts`, `SetupWizard.tsx`, `GenericStep.tsx`, 신규 `ExtensionSubWizard.tsx`/`visibility.ts`, 삭제 `ExternalKeysStep.tsx`(Phase 2).

---

## Phase 1 — Foundation (단수 → 복수 위자드, 그룹화)

> 의존성 없음. 키 메커니즘·action step 모두 건드리지 않음. 이 phase만으로 빌드·테스트·커밋 가능.

### Task 1: 코어 — `setup_wizard()` 복수 API + status 그룹화 + 네임스페이스 라우트

**Files:**
- Modify: `crates/oxipage-core/src/extension.rs` (트레이트 + 신규 타입)
- Modify: `crates/oxipage-core/src/setup.rs` (StatusResult, 핸들러, 라우트)
- Modify: `crates/oxipage-ext-profile/src/lib.rs` (마이그레이션)
- Test: `crates/oxipage-core/tests/setup_wizard.rs`

**Interfaces:**
- Produces: `pub fn setup_wizard(&self) -> Option<ExtensionWizard>` (트레이트, 기본 `None`); `pub struct ExtensionWizard { pub steps: Vec<SetupStep> }`; `pub struct ExtensionWizardInfo { extension_id, display_name, steps: Vec<ExtensionStepInfo> }`; `StatusResult.extension_wizards: Vec<ExtensionWizardInfo>`; 라우트 `POST /setup/extension-step/{ext_id}/{step_id}`.
- Consumes: 기존 `SetupStep`/`SetupField`/`SetupSaveHandler`/`PrefillSource` (변경 없음). `VisibilityRule` enum은 이 phase에서 타입만 정의(항상 `None` 사용) — Phase 3에서 실사용.

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
(`NoopSave`, `body_json`, `get_resp`, `build_app` 헬퍼는 기존 파일 패턴에 맞춰 추가/재사용. `NoopSave::save`는 이 phase에선 `Ok(())` — StepOutcome 반환은 Phase 3.)

- [ ] **Step 2: 테스트 실패 확인**

Run: `cargo test -p oxipage-core --test setup_wizard status_returns_multiple_steps_per_wizard`
Expected: 컴파일 에러 (`setup_wizard`/`ExtensionWizard` 미정의).

- [ ] **Step 3: 코어 타입 구현 (`extension.rs`)**

- `VisibilityRule` enum 정의: `FieldNotEmpty { step_id, field }`, `FieldEquals { step_id, field, value }`, `All(Vec)`, `Any(Vec)`. `#[derive(Debug, Clone, serde::Serialize)]` + `#[serde(tag="kind", rename_all="snake_case")]`. (이 phase에선 사용 안 함 — Phase 3.)
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

## Phase 2 — 키 흡수 (아웃라인, 미결정 사항 대기)

> **미결정 #1 (키 처리):** 공유 `external-keys` step을 폐지하고 각 확장 위자드로 키를 흡수할지. 승인 시 아래를 디테일화.

**예정 task (승인 후 TDD 디테일 작성):**
- **Task 3 (코어):** `SetupFieldKind::Secret` 추가; 트레이트에서 `external_api_keys()`/`save_external_key()` 제거; `ExternalApiKey`/`ExternalKeyScope` 타입 제거; `StatusResult.external_api_keys` 필드 + `setup_status_handler` 키 수집 로직 제거; `/setup/external-keys` 라우트 + `setup_external_keys_handler` + `ExternalKeysInput` 제거. (주의: 트레이트 메서드 제거가 movies/books/activity를 동시에 깨뜨리므로 Task 4와 연속 실행.)
- **Task 4 (확장 + 프론트):** movies/books/activity 각각 1-step `setup_wizard()`(Secret/text 키 필드, `persist_extension_config` 저장)로 전환; `GenericStep` Secret 렌더(password input, 프리필 금지); `ExternalKeysStep.tsx` 삭제 + `submitExternalKeys`/`external-keys` step 제거.

**대안(사용자가 공유 step 유지를 택할 경우):** 키 메커니즘 현행 유지, Phase 2 축소/생략.

---

## Phase 3 — 조건부 가시성 + action step (아웃라인, 미결정 사항 대기)

> **미결정 #2 (action step):** movies/books의 테스트/가져오기, activity 동기화 같은 action step을 v1에 넣을지. "조건부 step 포함"을 골랐으므로 action step은 그 조건값을 만드는 주체 — 사실상 함의. 승인 시 디테일화.

**예정 task (승인 후 TDD 디테일 작성):**
- **Task 5 (코어):** `SetupSaveHandler::save(...) -> Result<StepOutcome>`; `StepOutcome { values: Map }` + `from_form`; `setup_extension_step_handler`가 `DataEnvelope<StepOutcome>` 반환; `ExtensionStepInfo.visible_when` 실직렬화; `VisibilityRule` 평가는 클라이언트(코어는 직렬화만).
- **Task 6 (프론트):** 신규 `visibility.ts`(`evalRule` 순수 함수 + 단위 테스트); 신규 `ExtensionSubWizard.tsx`(내부 step 네비게이션 + outcomes 맵 + evalRule); `GenericStep` action 렌더(빈 fields → 실행 버튼); `SetupWizard`의 `extension-wizard` 케이스에서 서브-위자드 연결.
- **Task 7 (확장):** movies/books = `[키 → 테스트(action, visible: 키 있음) → 가져오기(action, visible: test.connection_ok=="true")]`; activity = `[사용자명 → 동기화(action)]`; profile save `from_form` 반환.

**대안(사용자가 action step 후순위를 택할 경우):** Phase 3은 `visible_when` 타입 슬롯만 열어두고(기본 항상 표시) 조건부 로직은 후속 이터레이션으로 연기.

---

## Self-Review (Phase 1 기준)

**1. Spec coverage (Phase 1):** D1(복수 위자드) → Task 1. 네임스페이스 라우트 → Task 1 Step 5. 프론트 그룹화 → Task 2. ✓ Phase 2/3은 미결정 사항 확정 후 디테일에서 cover.

**2. Placeholder scan:** Phase 1에 TODO/TBD 없음 — 모든 단계에 실제 시그니처/테스트 코드. Phase 2/3은 명시적으로 "아웃라인, 대기"로 표기(placeholder 아님 — 의도적 지연).

**3. Type consistency:** `setup_wizard()` 시그니처 Task 1 전체에서 일관. `visible_when`/`is_action` 필드가 Task 1(추가, 미사용) → Task 5/6(사용)로 이어짐. step id 명세 §8과 일치 예정.

**알려진 위험:**
- Phase 1은 외부 결정에 무관 — 즉시 실행 가능.
- Phase 2 Task 3이 workspace를 일시적으로 깨뜨리므로 Task 3→4 연속 실행 필요 (디테일화 시 명시).

## Execution Handoff

Phase 1은 즉시 실행 가능. 구현 시작 시: 별도 브랜치 분기 (`git checkout -b feat/extension-subwizards`). Phase 2/3은 두 미결정 사항 확정 후 디테일 채우고 이어서 실행.
