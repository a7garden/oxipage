# Console Reliability & Publishing Suite — 진행 상황 및 남은 작업

> **작성:** 2026-07-31
> **목표:** oxipage console의 라우팅/테마/프리뷰/배포/작성 UX 5개 서브프로젝트 완료

## 1. 현재 위치

**실행 중:** Subagent-Driven Development — Foundation plan (8개 task 중 4개 완료, Task 5 실행 중)

| Plan | 상태 | Task 수 |
|---|---|---|
| 1. Runtime/Routing Foundation | 🔄 **진행 중** | 4/8 완료 |
| 2. Admin Theme System | ⏳ 대기 | 13 |
| 3. Preview + Media | ⏳ 대기 | 9 |
| 4. GitHub Pages | ⏳ 대기 | 9 |
| 5. Authoring UX | ⏳ 대기 | 23 |

의존성: 1 → 2,3,4 → 5 (2-4는 1 완료 후 병렬 가능, 5는 2·3 완료 후)

## 2. 완료된 것 (커밋 포함)

| 항목 | 커밋 | 검증 |
|---|---|---|
| 설계 spec 6건 | `f41c370` | 문서 구조 검증 완료 |
| 구현 plan 5건 | `3afba88`, `414f467`, `161f141` | 총 9,419줄 / 62 task |
| **T1** 죽은 console embed/build.rs 삭제 | `b0ae099` | cargo build + 4/4 단위 테스트 |
| **T2** core build.rs dual-mode + revision marker | `9150031` | `.build-revision` 생성, missing admin.html panic 확인, `cargo package --list`에 `.build-revision` 포함 확인 |
| **T3** static cache/ETag/revision policy | `24824a2`, `6a89ca0`, `a270ef4`, `5b147ff` | cache_headers 3/3, fix round 1회(모두 해결) |
| **T4** AdminErrorBoundary | `a7e8d93` | tsc + bun build 성공, 리뷰 승인 |

**기타:** 외부 예약 태스크가 `fae82be`(doc base_url 변경), `9a4df04`(Cargo.toml 포맷) 커밋 — 무해, 진행에 영향 없음.

## 3. 진행 중

**Foundation T5: SiteContext resolved paths** (`a7e8d93` 이후 실행 중)
- SiteContext에 `project_dir/data_dir/out_dir/media_dir/startup_server` 추가 (기존 `path` 필드 → `project_dir`로 변경)
- `config: Arc<Config>`는 이번 task에서 유지 (T6에서 제거)
- Step 4b: 기존 깨져 있던 통합 테스트 4개 파일의 `SiteRegistry::new` arity 수정 (1-arg → 3-arg) — 이 사전 결함이 `cargo test -p oxipage-console` 컴파일을 막고 있었음

## 4. 남은 작업 상세

### Foundation (T5–T8)

- **T5** SiteContext resolved paths + 테스트 arity 수정 (실행 중)
- **T6** `MutableSiteSettings` + `config_write_lock` — `site_paths.rs` 신규, `ctx.config` reader 전부 `ctx.settings.read().await.*` / `ctx.startup_server.*`로 이전 후 `config` 필드 제거, `config_put`을 lock + reread + allowlist patch + atomic rename으로 교체
- **T7** 레거시 top-level build/deploy 라우트 제거 (`build/site_build.rs`, `deploy/site_deploy.rs` 삭제, router 정리)
- **T8** favicon 수정 + `/sites` deep-link e2e 검증 (curl로 cache header 확인)

### Admin Theme System (13 task)

- `oxipage-core/src/theme.rs` 단일 catalog (6테마 union)
- `GET /api/console/theme`을 console router로 이동 (default-site 해석)
- per-site theme GET/PUT을 공유 catalog로
- `web/theme-boot.js` 공용 FOUC boot (중복 inline script 제거)
- `shared/theme.ts` 재작성: `ConsoleAppearance`(system/light/dark) 3-state controller, `applyServerTheme`는 palette만 적용
- `ThemeToggle` 3-state 전환, Topbar 사설 toggle 삭제
- `AdminApp`에서 `applyServerTheme()` 호출
- SettingsPage Appearance 섹션
- ThemesPage가 서버 catalog 사용
- sidebar tokens를 `[data-theme]` scope로 이동, 하드코딩 hex 교체
- `--accent-hue`를 public-theme scope에서 실제 OKLCH accent로 연결

### Preview + Media (9 task)

- `oxipage-core/src/build_manifest.rs` — `BuildManifest` 타입 (build_id/deployment_base/theme_id/asset_revision/built_at) + read/write
- `build_writer.rs` — public asset 태그를 상대 경로(`assets/...`)로, `<base href>` 삽입, manifest 기록
- `preview/handler.rs` 재작성 — prefix-aware, directory-index, 404.html fallback, base href rewrite, 424, traversal 가드, no-store
- `media/` 모듈 — multipart 업로드 (JPEG/PNG/WebP/GIF, 10MiB, magic bytes, UUID, atomic rename) + live serving
- axum multipart feature 추가
- `web/src/shared/assets.ts` — `AssetResolver` 계약
- `ImageField.tsx` — URL/업로드/썸네일/클리어/에러
- DeployPage Preview Site 버튼
- `uploadImage` client 함수

### GitHub Pages (9 task)

- `DeployConfig`/`GitHubPagesTarget` config 연동 + `pages_url`/`base_path` 파생
- `deploy_github_pages` 재작성 — `repo_dir` 인자, `Command::current_dir`, bash -c 제거, UUID worktree, cleanup guard, origin 검증, `DeployOutcome`
- `operations.rs` — build+deploy 통합 SiteOperationGuard (409)
- `deploy_log` 히스토리 + `GET /deploys`, `GET /operations/current`
- preflight endpoint (gh 설치/인증/repo/base/theme 검사)
- DeployPage preflight 카드 + deploy history + reattach
- SettingsPage Deployment 섹션
- CLI `--site` 실제 사용

### Authoring UX (23 task)

- public presentation/view 컴포넌트 분리 (BlogPostView, ProfileView 등)
- `EditorPreviewDrawer` + `DraftPreviewPane` (실제 renderer 재사용, 저장 전 미리보기)
- `TagInput`, `validation.ts`, `ApiValidationError`, `DrawerField error prop`
- ProfileTab (14필드 + education/custom_links repeater + `expected_updated_at` 409)
- atomic reorder API 2종 (novels chapters, projects screenshots)
- extension별 수정: Blog(TagInput/언어), Books(status enum/ISBN), Projects(ImageField/links/날짜), Movies(TMDB search/series), Novels(ImageField/atomic reorder), Links(ImageField), Scraps(og override)
- 서버측 image/email/ISBN/date validation 추가
- dirty-form discard 경고

## 5. 실행 상태 (SDD)

- **워크스페이스:** `.superpowers/sdd/2026-07-31-console-runtime-routing-foundation-plan/` (ledger: `progress.md`)
- **스크립트:** `~/.omp/plugins/cache/plugins/superpowers-dev___superpowers___6.2.0/skills/subagent-driven-development/scripts/`
- **브랜치:** main (사용자 승인)
- **테스트 관행:** Rust `cargo test -p <crate>`, TS `cd web && npx tsc --noEmit` + `bun run build`
- **프로세스:** 각 task = implementer dispatch → review package → task reviewer → fix loop (최대 5라운드) → final whole-branch review

## 6. 리스크 / 주의

1. **외부 예약 태스크 동일 트리 공유** — 중간에 무관 커밋이 섞일 수 있음 (발생 시 무시, 내 파일만 stage)
2. **사전 결함:** console 통합 테스트 4개가 `SiteRegistry::new` arity drift로 컴파일 불가였음 — T5 Step 4b에서 수정 예정
3. **`git clean -fdx` 금지** — `.superpowers/sdd/` 워크스페이스가 git-ignored (지우면 ledger 소실, `git log`으로 복구 가능)
4. 다음 plan들의 인터페이스 계약은 foundation T6 결과(`MutableSiteSettings`, `startup_server`)에 의존 — T6 완료 후 Theme/Preview/Deploy dispatch
