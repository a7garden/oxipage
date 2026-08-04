# Console Reliability & Publishing Suite — 완료 보고

> **작성:** 2026-07-31
> **목표:** oxibuilder console의 라우팅/테마/프리뷰/배포/작성 UX 5개 서브프로젝트 완료
> **상태: ✅ 전 서브프로젝트 완료**

## 1. 최종 상태

| Plan | Task 수 | 상태 | 비고 |
|---|---|---|---|
| 1. Runtime/Routing Foundation | 8 | ✅ 완료 | 전원 커밋 + 검증 |
| 2. Admin Theme System | 13 | ✅ 완료 | 전원 커밋 + 검증 |
| 3. Preview + Media | 9 | ✅ 완료 | 전원 커밋 + 검증 |
| 4. GitHub Pages | 9 | ✅ 완료 | 전원 커밋 + 검증 |
| 5. Authoring UX | 23 | ✅ 완료 | 전원 커밋 + 검증 |

**검증:** `cargo build --workspace` green · Rust 테스트 132+ 통과 · `cd web && npx tsc --noEmit` clean · `bun run build` 성공 · 콘솔 스모크(`/sites` 200 text/html no-cache, `/api/console/sites` 200, `/api/console/theme` default-site 정의 반환, `/theme-boot.js` 200) 통과.

## 2. 커밋 요약 (5개 plan 주요 커밋)

- Foundation: embed 통합, dual-mode build.rs, cache/ETag/revision, ErrorBoundary, SiteContext resolved paths, MutableSiteSettings + config_write_lock, 레거시 라우트 제거, favicon + e2e
- Theme: `oxibuilder_core::theme` 카탈로그, default-site `/api/console/theme`, theme-boot.js, 3-state appearance, `[data-theme]` scoped tokens, `--accent-hue` OKLCH
- Preview/Media: `BuildManifest` + `derive_deployment_base`, build_writer relative tags + `<base>`, prefix-aware preview handler(424/404 fallback/base rewrite), multipart media upload(magic bytes/10MiB/UUID), AssetResolver + ImageField + Preview Site 버튼
- GitHub Pages: `DeployConfig`/`GitHubPagesTarget` 검증+URL/기반 파생, atomic deploy config patch, repo-scoped `deploy_github_pages`(UUID worktree/RAII), `SiteOperationGuard` 단일 슬롯, deploy_log 히스토리, preflight/current APIs, CLI `--site`, DeployPage preflight+history+reattach, Settings/Dashboard 배포 섹션
- Authoring: `ApiValidationError` + `jsonOrThrow` field 보존, validation.ts, TagInput, EditorPreviewDrawer/DraftPreviewPane, `*View`/`*Card` 분리, Profile CRUD(`expected_updated_at` 409), novels/projects atomic reorder, `oxibuilder_core::validation`, 7개 탭 개선(TagInput/ImageField/검증), blog 서버 검증

## 3. 실행 방식 메모

- 워크스페이스: `main` 브랜치 (사용자 승인)
- 병렬 서브에이전트 디스패치 2회 실패(파일 되돌림 충돌 + 토큰 한도) → 이후 전량 메인 에이전트 인라인으로 복구/완료. 인라인 진행이 안정적이었음.
- 외부 예약 태스크의 무관 커밋(`d369ae0` project-oxi 마이그레이션 등)은 그대로 두고 충돌 없이 진행.

## 4. 남은 작업 / 후속 제안 (비차단)

> **업데이트 (2026-07-31):** 항목 3–6은 `2026-07-31-console-followup-design.md` 스펙으로 해결 완료 (커밋 `acd8b50`, `81b6c90`, `bf73b94`, `113902c`). 항목 1–2만 남음.

1. **T23 스모크(Profile first-write `expected_updated_at=""`)** — 코드는 완료(T11의 `is_empty()` 가드), 실제 브라우저 스모크만 미실행. `bun run dev`로 신규 사이트 Profile 탭 첫 저장 확인 필요.
2. **배포 실측 스모크** — plan T9 Step 6(두 개 임시 사이트+리모트로 root/project Pages 배포 검증)은 git remote/gh 자격 증명 필요로 미실행. `deploy_github_pages` 단위 테스트(manifest mismatch, origin match)는 통과.
3. **BooksTab 상태 enum** — ✅ 해결: 읽기 시 서버 정규화(`read`→`completed`, `dnf`→`dropped`, repo 반환 5개 지점) + 단위/통합 테스트. DB 마이그레이션 없음.
4. **MoviesTab TMDB 검색** — ✅ 해결: 키 미설정 시 `tmdb_disabled` 503을 감지해 인라인 힌트 + Settings 링크 표시. `ApiError.code` 보존 추가.
5. **빌드 경고** — ✅ 해결: `if let Some(s)` → `if status.is_some()` 2건. 경고 0건 확인.
6. **`/preview/{slug}/` 미등록 사이트 시 admin.html fallback** — ✅ 해결: top-level `/preview/{*rest}` 404 라우트 추가(`preview_missing`). canonical `/api/console/preview/...` 불변 테스트 확인.

## 5. 검증 명령

```bash
cargo build --workspace
cargo test -p oxibuilder-core -p oxibuilder-console -p oxibuilder -p oxibuilder-deploy -p oxibuilder-ext-profile -p oxibuilder-ext-novels -p oxibuilder-ext-projects -p oxibuilder-ext-blog -p oxibuilder-ext-books -p oxibuilder-ext-movies -p oxibuilder-ext-links -p oxibuilder-ext-scraps
cd web && npx tsc --noEmit && bun run build
```
