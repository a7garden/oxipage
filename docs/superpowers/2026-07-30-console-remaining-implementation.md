# Console — Remaining Implementation (S3–S5)

> **Status:** 2026-07-30, S2 (Extension Gaps) 완료. S3/S4/S5 미구현.
> **완료된 S2:** Movies group update/delete, Projects screenshot update, WASM registry list (백엔드) + Novels 챕터, Movies 시리즈, Projects 스크린샷, Extensions 레지스트리 UI (프론트엔드)

---

## S3: Deploy Pipeline (7 tasks)

**의존성:** `oxibuilder-core` crate 수정 필요. SSE/async 인프라 구축 후 프론트엔드 연결.

| # | 태스크 | 파일 | 상태 |
|---|--------|------|------|
| 1 | Core streaming build (`build_site_with_progress`) | `crates/oxibuilder-core/src/build.rs` | ❌ |
| 2 | Console deps + BuildRun + BuildGuard 인프라 | `Cargo.toml` (+tokio-stream, dashmap, uuid), `build/build_run.rs` 신규 | ❌ |
| 3 | Build endpoints: POST 202 + GET /stream SSE | `per_site.rs`, `build/site_build.rs` | ❌ |
| 4 | `oxibuilder-deploy` crate (CLI deploy 로직 추출) | `crates/oxibuilder-deploy/` 신규, workspace member | ❌ |
| 5 | Deploy endpoints (stub → 실제 gh-pages) | `deploy/site_deploy.rs`, `per_site.rs` | ❌ |
| 6 | 프론트: DeployPage SSE 로그 패널 | `web/src/admin/deploy/DeployPage.tsx` | ❌ |
| 7 | Wire + smoke | — | ❌ |

**참고:** S3 spec = `docs/superpowers/specs/2026-07-30-console-deploy-pipeline-design.md`
S3 plan = `docs/superpowers/plans/2026-07-30-console-deploy-pipeline-plan.md`

---

## S4: Settings Residual (3 tasks)

**작음.** set_default stub 수정 + 프론트 연결. Purge All Data 연기.

| # | 태스크 | 파일 | 상태 |
|---|--------|------|------|
| 1 | `SiteRegistry::set_default` 구현 + `set_default` 핸들러 수정 | `sites_runtime.rs`, `router.rs` | ❌ |
| 2 | 프론트: SitesPage "Set as default" + SettingsPage 기본 사이트 셀렉터 | `web/src/admin/sites/SitesPage.tsx`, `web/src/admin/settings/SettingsPage.tsx` | ❌ |
| 3 | Smoke test | — | ❌ |

**참고:** S4 spec = `docs/superpowers/specs/2026-07-30-console-settings-residual-design.md`

---

## S5: Console Global UX (6 tasks)

**프론트엔드 전용.** S3의 `build_log.finished_at` 항목은 S3에 흡수됨. 반응형(#1)은 연기.

| # | 태스크 | 파일 | 상태 |
|---|--------|------|------|
| 1 | CSS tokens: `--console-sidebar-*` 변수 추가 + Sidebar/Topbar 하드코딩 hex 치환 | `tokens.css`, `Sidebar.tsx`, `Topbar.tsx` | ❌ |
| 2 | Scroll reset: `ScrollToTop` 컴포넌트 | `shared/ui/ScrollToTop.tsx` 신규, `App.tsx` 마운트 | ❌ |
| 3 | Offline indicator: `OfflineBanner` 컴포넌트 | `shared/ui/OfflineBanner.tsx` 신규, `ConsoleShell.tsx` 마운트 | ❌ |
| 4 | Markdown preview: `marked` 의존 + `MarkdownEditor` 컴포넌트 + content tab body 치환 | `package.json`, `shared/ui/MarkdownEditor.tsx`, `content/*Tab.tsx` | ❌ |
| 5 | SiteSelector info panel: URL + stats | `web/src/admin/shell/SiteSelector.tsx` | ❌ |
| 6 | Full check + smoke | — | ❌ |

**참고:** S5 spec = `docs/superpowers/specs/2026-07-30-console-global-ux-design.md`

---

## 요약

| 서브프로젝트 | 태스크 수 | 완료 | 미완료 | 예상 난이도 |
|-------------|----------|------|--------|-----------|
| S2 (Extension Gaps) | 9 | 9 | 0 | ✅ |
| S3 (Deploy Pipeline) | 7 | 0 | 7 | 중-상 (SSE, deploy) |
| S4 (Settings Residual) | 3 | 0 | 3 | 하 |
| S5 (Global UX) | 6 | 0 | 6 | 하-중 |
| **합계** | **25** | **9** | **16** | |

## 파일 경로 (커밋된 spec + plan)

- `docs/superpowers/specs/2026-07-30-console-extension-gaps-design.md` (S2)
- `docs/superpowers/specs/2026-07-30-console-deploy-pipeline-design.md` (S3)
- `docs/superpowers/specs/2026-07-30-console-settings-residual-design.md` (S4)
- `docs/superpowers/specs/2026-07-30-console-global-ux-design.md` (S5)
- `docs/superpowers/plans/2026-07-30-console-extension-gaps-plan.md` (S2)
- `docs/superpowers/plans/2026-07-30-console-deploy-pipeline-plan.md` (S3)
- `docs/superpowers/plans/2026-07-30-console-settings-residual-plan.md` (S4)
- `docs/superpowers/plans/2026-07-30-console-global-ux-plan.md` (S5)
