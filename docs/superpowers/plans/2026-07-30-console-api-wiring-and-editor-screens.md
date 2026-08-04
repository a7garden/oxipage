# Console — Remaining Work

> 2026-07-30. Console shell redesign + API wiring + content edit screens 구현 완료 상태에서
> 남은 작업을 정리한다. 이 문서는 `specs/2026-07-30-console-shell-redesign.md`와
> 현재 구현 코드 기준의 갭 분석이다.

---

## Phase 7: Search / Filter (모든 Content Tab)

**Problem:** 7개 content tab 모두에 `placeholder="Search..."` `<Input>`이 렌더링되지만
어떤 검색 로직에도 연결되지 않았다.

**Task:**
모든 tab에서 search input을 client-side filter에 연결:

- Blog (by title)
- Projects (by title_ko/title_en)
- Links (by title/url)
- Movies (by title)
- Books (by title/author)
- Novels (by title)
- Scraps (by title/source_url)

또는 각 extension의 server-side search (`?q=`)를 활용.
Scraps/Books/Movies는 API에 search가 없음 — extensions 수정 필요.

**Files:** `web/src/admin/content/*Tab.tsx` (7 files)

---

## Phase 8: Extension-server Gaps

### 8.1 Novels — Chapter 관리

**Problem:** NovelsTab에는 chapter CRUD가 전혀 없다.
Novel drawer는 제목/시놉시스/커버만 입력 — chapter 생성/편집/순서변경/publish 불가.

**Task:**
- Chapter list view (novel별 nested table or expandable row)
- Chapter drawer: title + body + order + publish
- Route: `/{novelSlug}/chapters` (GET list), `/{slug}/chapters/draft` (GET drafts),
  `/{slug}/chapters/{order}` (PATCH/DELETE), `/{slug}/chapters/{order}/publish` (POST)
- `contentClient`에 `chapters: { list, create, update, delete, publish }` 추가

**Backend:** `/novels/{slug}/chapters`, `/novels/{slug}/chapters/{order}` — routes exist in `oxibuilder-ext-novels`, SPA만 추가.

### 8.2 Movies — Series Group 관리

**Problem:** MoviesTab에 "New Series" 버튼이 있지만 아무 동작도 하지 않는다.

**Task:**
- SeriesGroup drawer: title_ko/title_en, slug, cover_image
- Series group list view (groups as collapsible sections)
- Route: `/movies/series` (GET list, POST create), `/movies/series/{slug}` (GET group detail)

**Backend:** Routes exist in `oxibuilder-ext-movies`.

### 8.3 Projects — Screenshot 관리

**Problem:** Projects drawer에 screenshot 업로드/관리가 없다.
Backend API는 `/{slug}/screenshots` (POST), `/{slug}/screenshots/{sid}` (DELETE) 존재.

**Task:**
- Screenshot list in project drawer
- URL 입력 + delete

### 8.4 Extensions — "Available" 섹션

**Problem:** 이전 ExtensionsPage에는 Installed + Available 섹션이 있었지만,
현재 per-site endpoint는 모든 extension을 하나의 리스트로 내려준다.
"Install extension" 버튼도 없다.

**Task:**
- 모든 extension은 static link이므로 "available"은 비어 있음 (WASM만 installable)
- WASM install UI (`.wasm` 업로드 또는 레지스트리 브라우징)
- `POST /api/console/extensions/install` — 이미 core에 있음
- "Install Extension" 모달 or drawer

---

## Phase 9: Deploy Pipeline

### 9.1 Build Log Streaming

**Problem:** `POST /api/console/s/{slug}/build`는 SSG 결과만 반환하고 로그를 스트리밍하지 않는다.
DeployPage는 build_log 테이블에서 page_count/out_dir만 보여줌.

**Task:**
- Build 실행 시 콘솔에 로그를 WebSocket 또는 SSE로 전달
- DeployPage에 실시간 로그 뷰어 (terminal-style monospace)
- `GET /api/console/s/{slug}/build/{buildId}/log` polling fallback

**Backend:** `oxibuilder-console/src/per_site.rs` build handler 수정.
Build output capture + `build_log`에 log_text 칼럼 추가 또는 별도 로그 파일.

### 9.2 Deploy Action 구현

**Problem:** `POST /api/console/s/{slug}/deploy`는 stub — "use `oxibuilder deploy --site <slug>`" 메시지.

**Task:**
- Git worktree + gh-pages deploy 로직을 CLI에서 console로 이전
- 또는 console이 `oxibuilder deploy` CLI를 subprocess로 호출
- Deploy 상태 폴링: `GET /api/console/s/{slug}/deploy/{id}/status`

**Backend:** `oxibuilder-cli/src/commands/deploy.rs`에 구현된 로직을 console에서 재사용.

### 9.3 Build/Deploy Trigger UX

**Problem:** Build/Deploy 버튼이 Dashboard + DeployPage 양쪽에 있지만
빌드 중 상태 표시와 결과 피드백이 부족함.

**Task:**
- Build 진행 중에는 버튼 비활성화 + spinner
- 완료 후 build_log 자동 refetch
- Build 실패 시 에러 메시지 표시 (현재는 `catch`에서 `alert`)

---

## Phase 10: Settings 확장

### 10.1 Danger Zone 활성화

**Problem:** "Purge All Data" / "Delete Site" 버튼이 `disabled`.

**Task:**
- `DELETE /api/console/s/{slug}` (site + data 삭제) — endpoint 없음. 추가 필요.
- "Purge All Data" → 각 extension의 데이터 purge 후 site 유지
- 둘 다 confirm dialog 필요

### 10.2 Integrations 편집

**Problem:** Settings의 Integrations 섹션은 read-only로 env variable 이름만 표시.

**Task:**
- PUT config에서 integrations 업데이트 지원 (github_username, tmdb_api_key_env, aladin_ttbkey_env 입력 필드)
- 서버: `per_site.rs config_put`에 integrations 처리 추가

### 10.3 Language 관리

**Problem:** Site 설정에서 language 선택이 `["ko", "en"]` 두 개만.

**Task:**
- `languages: string[]` — 다중 선택 UI (checkbox group or multi-select)
- `default_lang` — dropdown (선택된 languages 중에서)

### 10.4 Data Dir 표시

**Problem:** Spec §4.6에 Data Dir read-only 필드 그리드가 명시됨.

**Task:**
- SettingsPage에 `data_dir` read-only 입력 추가
- 서버: `/config` 응답에 이미 포함됨

---

## Phase 11: 대시보드 확장

### 11.1 Storage / Uptime / Extensions 통계

**Problem:** Dashboard stat cards 4개 중 3개가 `counts`를 사용하지만
Storage와 Uptime은 아직 없음. Extensions 설치는 `listExtensions`로 가져올 수 있음.

**Task:**
- StatCard 4열 → Storage(disk usage), Extensions 활성화 수
- `GET /api/console/s/{slug}/stats` 엔드포인트 (또는 개별 aggregation)

### 11.2 Recent Posts → Cross-extension

**Problem:** Dashboard recent posts는 blog만 표시. Spec은 cross-extension recent 목록을 명시.

**Task:**
- 각 extension의 최근 항목을 병합 → 최신순 정렬
- Source 확장 표시 (Badge or chip: "Blog", "Project", "Movie Review" 등)

### 11.3 배포 시간 / 상태 표시

**Problem:** Spec §4.1에 "Last deployed 2 hours ago" 라인.

**Task:**
- Dashboard header에 마지막 배포 시간 표시
- `listBuilds` 결과에서 첫 번째 build의 `created_at` 사용

---

## Phase 12: 콘솔 전역 UX

### 12.1 반응형 레이아웃 (Tablet)

**Problem:** v1은 desktop-only. Tablet(768-1023px)에서 sidebar가 200px을 차지하면
content 영역이 너무 좁아짐. Spec §8에 명시됨.

**Task:**
- Tablet: sidebar를 56px icons-only 모드로 축소 (tooltip으로 label 표시)
- Tailwind `md:` breakpoint로 처리

### 12.2 SiteSelector Info 패널

**Problem:** Spec §2.3에 우측 info 패널이 명시됐지만 구현되지 않음.
사이트 목록만 있고 "Console info / current site info" 우측 컬럼이 없음.

**Task:**
- SiteSelector dropdown을 2-column 레이아웃으로 확장
- 우측: oxibuilder 버전, extension 활성화 수, 사이트 상태

### 12.3 Offline / Unavailable Indicator

**Problem:** Spec §9에 "API unavailable → topbar indicator" 명시.

**Task:**
- TanStack Query 전역 `onError` 핸들러
- Topbar에 빨간 연결 상태 dot
- 연속 에러 시 5분 polling 중단

### 12.4 Site 전환 시 Scroll Reset

**Problem:** 사이트 전환 시 content 영역 scroll 위치가 유지됨.
새 사이트 로드 시 scroll을 top으로 리셋해야 함.

**Task:**
- React Router `useLocation` 감지 → `window.scrollTo(0, 0)`
- 또는 `<ScrollRestoration>` 확장

---

## Phase 13: 서버 사이드 신규 엔드포인트

### 13.1 `GET /api/console/s/{slug}/stats`

**Problem:** Spec §7.1에 명시됐지만 구현되지 않음.
Dashboard가 "Posts", "Projects", "Books", "Links" count를 개별 fetch하는 건 비효율적.

**Task:**
- 각 extension 테이블에서 count 한 번에 조회
- Response: `{ posts, projects, links, books, movies, novels, scraps }` + storage bytes

### 13.2 `GET /api/console/s/{slug}/content/recent`

**Problem:** Spec §7.1에 명시됨. Cross-extension 최근 항목.

**Task:**
- 확장별 `published_at DESC LIMIT 3` UNION ALL → 최신순 정렬 LIMIT 10
- Response: `{ content: [{ ext_id, title, slug, published_at, type }] }`

### 13.3 `DELETE /api/console/s/{slug}`

**Problem:** Site 삭제 endpoint가 없음. `removeSite`는 서버에 `/{slug}` DELETE 없음.

**Task:**
- `REGISTRY`에서 slug 제거, sites.toml 갱신, DB+data_dir 삭제
- 또는 "삭제" 대신 "등록 해제" (data_dir 유지)

### 13.4 Server-side search (content tabs)

**Problem:** Content tabs의 search input이 작동하지 않음. Core `/api/console/search`는
public site용 FTS search — 확장별 필터링 불가.

**Task:**
- 각 extension의 list endpoint에 `q:` 쿼리 파라미터 추가 (blog는 title LIKE, projects는 title_ko/title_en LIKE)
- 또는 통합 search: `/api/console/s/{slug}/search?q=&ext=blog,projects`

---

## Phase 14: Chores

- **Sidebar footer**: 현재 `v1.0.0 · {slug}` — extension 개수나 활성화 수를 표시하지 않음 (ext count from listExtensions)
- **Duplicate `created_at` in BuildRecord**: per_site.rs에서 built record 생성 시
  `CREATE TABLE build_log ... created_at DEFAULT (datetime('now'))`로 처리하는데
  INSERT 시 `created_at`을 명시하지 않으면 `current timestamp` vs `datetime('now')` 차이로
  UTC/Local time 불일치 가능성 있음
- **Unused variable warnings**: `books/src/repo.rs`, `projects/src/repo.rs`에서 `status` 파라미터
  `s`가 `?draft=true` 분기에서 unused — `_s`로 수정
- **Spec §5.3 console tokens CSS**: `tokens.css`에 console 전용 dark sidebar css variable
  추가가 명시됐지만, Sidebar는 inline style (`style="backgroundColor: #1a1e24"`)로 처리됨.
  CSS variable로 migration 가능
- **Markdown editor**: Blog body는 `<Textarea rows={16} className="font-mono">`로
  raw markdown 편집. Markdown preview 탭이나 WYSIWYG 편집기는 없음.
  서버 사이드 `markdown-it`이 이미 `package.json`에 있음.

---

## v1 Scope Out (Spec §13)

Spec에서 명시적으로 v1 범위 밖으로 선언된 항목들.
진행 전 재확인 필요:

- [ ] Tablet/mobile responsive layout
- [ ] 실시간 WebSocket build log streaming
- [ ] 테마 커스텀 에디터 (OKLCH GUI — theme_id 리스트만 있고 커스텀 OKLCH 생성 불가)
- [ ] 확장 마켓플레이스 GUI (현재 unavailable, WASM only)
- [ ] 멀티 유저 인증
- [ ] 백업 스케줄링 UI
