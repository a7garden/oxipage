# Site-Picker Console — 남은 작업 정리

> 마지막 커밋: `b172a81` (브랜치 `feat/site-picker-console`)
> 작성: 2026-07-30

## 완료된 작업

| 작업 | 상태 | 변경 요약 |
|------|------|-----------|
| **T1** SitesFile 스키마 + SiteRegistry | ✅ | path-only, `oxibuilder-core`로 이동, SiteLoader |
| **T2** console.db + setup_state | ✅ | `~/.config/oxibuilder/console.db`, 마이그레이션 |
| **T3** Site-prefixed 라우터 + 미들웨어 | ✅ | `/s/{slug}/build,deploy`, `SiteScopedDb` 주입 |
| **T4+T8** 전체 확장 핸들러 전환 | ✅ | 9개 확장 `State<AppState>` → `Extension<SiteScopedDb>` |
| **T5** Admin SPA 골격 | ✅ | `web/src/admin/` + dual vite + RustEmbed |
| **T6 (백엔드)** create-site 핸들러 | ✅ | `POST /api/console/setup/create-site` |

## 남은 작업 (우선순위 순)

### 🔴 T6 — 위저드 UI 연동 (전면)

**해야 할 일:**

1. **`web/src/setup/StepSite.tsx`** — 사이트 디렉토리 입력 UI
   - 현재: 사이트명/URL 입력 (v1 레거시)
   - 변경: 경로 입력 (`~/oxibuilder/blog`) + "신설" / "기존 등록" 선택
   - POST `/api/console/setup/create-site` 호출
   - 응답의 `slug`를 이후 step에 전달

2. **`web/src/setup/StepDone.tsx`** — 완료 후 리다이렉트
   - 현재: `/` (퍼블릭 로비)로 이동
   - 변경: `/s/{slug}/`로 이동 (콘솔 대시보드)

3. **`web/src/setup/api.ts`** — `createSite()` 추가

**참고 파일:**
- `web/src/setup/StepSite.tsx`
- `web/src/setup/StepDone.tsx`
- `web/src/setup/api.ts`
- `web/src/setup/SetupWizard.tsx` (step 순서/흐름)

### 🔴 T7 — `:8788` / `run_admin` / `admin-web/` 제거

**해야 할 일:**

1. **`crates/oxibuilder-console/src/admin/mod.rs`** 삭제
   - `pub mod admin;`를 `lib.rs`에서 제거
   - `sites_api.rs`, `themes.rs`도 함께 삭제
   - `Embedded SPA` 관련 `Assets` (`admin-web/dist`) 제거

2. **`admin-web/` 디렉토리** 삭제
   - 모든 파일 `git rm`
   - `build.rs`의 `admin-web/dist` 참조 제거 (이미 `web/dist`로 변경됨)

3. **`crates/oxibuilder-cli/src/commands/init_console.rs`**
   - `--admin-port` 인자 제거
   - `Command::Admin` 정리

4. **`grep -rn "8788\|OXIBUILDER_ADMIN_PORT\|run_admin"`** 로 잔재 확인

**주의:** `setup_gate`와 `extension_gate` 미들웨어가 콘솔 라우트를 커버하지 않음 (T6에서 남긴 이슈). 콘솔 라우트는 `build_app`의 `api` 라우터가 아닌 별도 `nest("/api/console", ...)`로 마운트되므로, `setup_gate`(loopback 체크)와 `extension_gate`(비활성 확장 차단)의 적용을 받지 않음. 이는 보안 경계가 약해짐을 의미 — 콘솔 라우트도 같은 게이트를 적용해야 함.

### 🟡 T9 — Build/Deploy/Preview

**해야 할 일:**

1. **Build 트리거** — `POST /api/console/s/{slug}/build`
   - 현재: stub (`{"ok":true}` 반환)
   - 필요: `SiteRegistry.ctx_for(slug)`로 `SiteContext` 획득 → `build_site()` 호출 → `out/` 디렉토리 생성
   - `oxibuilder_core::build::build_site()`와 `write_static_outputs()` 호출

2. **Deploy 트리거** — `POST /api/console/s/{slug}/deploy`
   - 현재: stub
   - 필요: 기존 `oxibuilder deploy` CLI 로직 재사용

3. **Preview 핸들러** — `GET /api/console/preview/:slug/*`
   - 현재: 없음
   - 필요: `ctx.path.join("out").join(rest)` 파일 읽어서 반환
   - MIME 타입은 `mime_guess`

**참고 파일:**
- `crates/oxibuilder-console/src/router.rs` (라우트 등록)
- `crates/oxibuilder-console/src/build/` (신규 모듈)
- `crates/oxibuilder-console/src/deploy/` (신규 모듈)
- `crates/oxibuilder-console/src/preview/` (신규 모듈)

### 🟢 Cleanup

**해야 할 일:**

1. `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings` 통과 확인
2. `cd web && bun run build` 통과 확인 (TypeScript 오류 수정)
3. `cargo build --release --workspace` 로 릴리스 빌드
4. `oxibuilder site add blog --path /tmp/blog` 로 사이트 등록
5. `oxibuilder console` 실행 → `:8787` 접속 확인
6. `curl /api/console/sites` 가 JSON 반환하는지 확인
7. README 업데이트: `:8788` 제거, `oxibuilder site add --path` 설명

---

## 보안/구조 이슈 (v2.0 이후)

- **콘솔 라우트 게이트 누락**: `setup_gate`(loopback 전용)와 `extension_gate`(비활성 확장 차단)가 콘솔 라우트(`/s/{slug}/*`, `/sites/*`)에 적용되지 않음. T7에서 `app.nest("/api/console", console_router)`를 `build_app` 내부로 이동하고, 게이트 미들웨어를 해당 nest에 추가해야 함.
- **프론트엔드 미완성**: `web/src/admin/`의 페이지 컴포넌트들(Dashboard, Content, Extensions 등)이 stub 상태. 실제 admin SPA로 동작하려면 TanStack Query로 API 연동 + UX 완성이 필요.
- **WASM 확장**: `route_dispatcher()`가 Some인 WASM 확장은 console router에서 제외됨 (`ext.routes()` 대신 폴백). `api_fallback`이 이들을 처리하는데, 콘솔 라우트에서는 이 폴백이 없음.

---

## 파일 맵 (변경 대상)

```
crates/oxibuilder-console/
├── src/
│   ├── lib.rs              # app.nest("/api/console", console_router) → 게이트 추가
│   ├── router.rs           # build/deploy/preview 라우트 추가
│   ├── build/              # site_build.rs (T9 신규)
│   ├── deploy/             # site_deploy.rs (T9 신규)
│   ├── preview/            # handler.rs (T9 신규)
│   ├── admin/
│   │   ├── mod.rs          # T7: 삭제
│   │   ├── sites_api.rs    # T7: 삭제
│   │   └── themes.rs       # T7: 삭제
│   └── console_state.rs    # setup_complete 마킹

web/src/
├── admin/                  # T5 골격 (T6 연동 필요)
├── setup/
│   ├── StepSite.tsx        # 사이트 디렉토리 입력 UI (T6)
│   ├── StepDone.tsx        # /s/{slug}/ 리다이렉트 (T6)
│   └── api.ts             # createSite() (T6)

admin-web/                  # T7: 디렉토리 삭제
```
