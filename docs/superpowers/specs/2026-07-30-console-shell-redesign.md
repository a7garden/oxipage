# Console Shell Redesign — Design

> 2026-07-30. 기존 `site-picker-console-design` (backend architecture, site registry, setup flow)을 전제로,
> 콘솔의 **frontend shell + 페이지 레이아웃 + 콘텐츠 관리 UX**를 완전히 재설계한다.

## 0. 문제 진단

| 현재 | 문제 |
|------|------|
| `SiteShell.tsx` | 사이트 이름을 수평 nav 링크로 나열. 사이트 3개 이상이면 overflow. 실제로는 GCP 콘솔처럼 dropdown selector가 맞는 구조 |
| `DashboardPage` | `"대시보드 — 준비 중"` placeholder. 연결된 데이터 없음 |
| `content/`, `dashboard/`, `extensions/`, `themes/`, `deploy/` | 전부 빈 디렉토리. 페이지 skeleton조차 없음 |
| 라우팅 | `/s/:slug` → DashboardPage 단 하나. 하위 페이지 없음 |
| shell | sidebar 없음. header가 모든 링크를 담고 있음 |
| 디자인 토큰 | 공개 사이트용 OKLCH 토큰이 콘솔에 그대로 적용 — 콘솔은 어두운 sidebar가 필요 (현재 design doc §12에도 명시) |

## 1. 목표

- G1. **GCP 콘솔 스타일 사이트 스코프드 콘솔**. 사이트 selector dropdown → sidebar nav → context-sensitive page.
- G2. **모든 콘텐츠 관리 기능을 콘솔에서 수행**. 각 extension이 자신의 content management view를 소유하되, 콘솔이 unified tabs로 제공.
- G3. **기능하는 대시보드**. 통계 카드 + 최근 게시물 + 빠른 액션.
- G4. **Extensions, Themes, Deploy, Settings** 각 페이지가 실제 CRUD/액션을 제공.
- G5. 기존 `web/src/admin/` 코드베이스 내에서 구현. `admin-web/` 별도 프로젝트는 폐기.

## 2. 레이아웃

```
┌──────────────────────────────────────────────────────────────────┐
│ [oxibuilder] │ [selfhost ▾] │ My Blog  http://...    │  🌙  ⚙     │  topbar (48px)
├────────────┼─────────────────────────────────────────────────────┤
│            │                                                     │
│ ● Dash     │  ┌─ Page Title ───────────────────────────────┐     │
│ ☰ Content  │  │                                              │     │
│ ◆ Ext      │  │  [Page-specific content]                    │     │
│            │  │                                              │     │
│ ◐ Themes   │  └──────────────────────────────────────────────┘     │
│            │                                                     │
│ ↻ Deploy   │                                                     │
│ ⚙ Settings │                                                     │
│ ────────── │                                                     │
│ v1.0.0     │                                                     │
└────────────┴─────────────────────────────────────────────────────┘
 sidebar (200px)             main (flex-1)
  dark bg                    light bg
```

### 2.1 Topbar

- `oxibuilder` 로고 (좌측 고정)
- **Site selector dropdown**: 녹색 online indicator + 현재 사이트 slug + ▽ chevron.
  클릭 시 사이트 목록 패널 — 각 사이트에 상태 dot, 이름, URL. "Manage Sites", "Add New Site" 액션.
- Divider + 현재 사이트 title / URL 표시 (context 확인용)
- 우측: theme toggle, avatar (or settings icon)

### 2.2 Sidebar

Dark background (`#1a1e24`), 200px width.

```
General
  ◉ Dashboard
  ☰ Content
  ◆ Extensions
Appearance
  ◐ Themes
Operations
  ↻ Deploy
  ⚙ Settings
───────────────
 v1.0.0 · 8 ext  (footer)
```

- **Active item**: green left border (`#22c55e`) + slightly green bg tint + white text
- **Inactive**: gray text, transparent border
- **Hover**: lighten bg slightly
- Labels: uppercase, dark gray, 10px

### 2.3 Site Selector Dropdown

```
┌─────────────────────────────────┬──────────────────┐
│ Your Sites                      │ Info             │
│                                 │                  │
│ ● selfhost                     ✓│ oxibuilder v1.0.0   │
│   http://127.0.0.1:8787         │ uptime: 14d 3h   │
│                                 │ ext: 8 active    │
│ ● alibaba                       │                  │
│   https://blog.alibaba.com      │ Click to switch   │
│                                 │                  │
│ ● flyio                         │                  │
│   https://oxibuilder.fly.dev       │                  │
│                                 │                  │
│ [Manage Sites] [+ Add New Site] │                  │
└─────────────────────────────────┴──────────────────┘
```

- 좌측 2/3: 사이트 목록 (active 항목에 ✓ check)
- 우측 1/3: 콘솔/현재 사이트 info
- 하단: 액션 버튼
- 빨간 dot = unreachable / offline

## 3. 라우팅

```
/                          → HomeRedirect (→ /s/{default} 또는 /sites)
/sites                     → SitesPage (사이트 목록 CRUD)
/sites/new                 → NewSiteWizardPage
/s/:slug                   → DashboardPage
/s/:slug/content           → ContentPage (tabs)
/s/:slug/extensions        → ExtensionsPage
/s/:slug/themes            → ThemesPage
/s/:slug/deploy            → DeployPage
/s/:slug/settings          → SettingsPage
```

### 컴포넌트 트리

```
AdminShell
  └─ SiteShell (topbar + sidebar layout)
       ├─ Outlet
       │   ├─ DashboardPage
       │   ├─ ContentPage
       │   ├─ ExtensionsPage
       │   ├─ ThemesPage
       │   ├─ DeployPage
       │   └─ SettingsPage
       └─ (sidebar items link to /s/{slug}/{section})
```

### Data flow

- `useParams().slug` → 모든 API call에 `siteScopedFetch(slug, path)` 사용
- `api.ts`의 `siteScopedFetch` (`/api/console/s/{slug}{path}`) 이미 존재
- 각 페이지는 `slug`를 TanStack Query key에 포함시켜 사이트 전환 시 자동 refetch

## 4. 페이지 상세

### 4.1 DashboardPage

**데이터 소스**: (v2 기준) 사이트 통계는 확장별 post count aggregation 필요. v1 구현 시 mock 데이터로 시작 후 연결.

```
┌─ Page Header ───────────────────────────────────────────────────┐
│ Dashboard                              [↻ Rebuild] [⇧ Deploy]  │
│ selfhost · Last deployed 2 hours ago                            │
└──────────────────────────────────────────────────────────────────┘

┌───────────┐ ┌───────────┐ ┌───────────┐ ┌───────────┐
│ Extensions│ │ Posts     │ │ Storage   │ │ Uptime    │
│    8      │ │   42      │ │  2.3 MB   │ │  99.8%    │
│ all active│ │ +3 this wk│ │12MB avail │ │14d uptime │
└───────────┘ └───────────┘ └───────────┘ └───────────┘

Recent Posts
┌──────────────────────────────────────────────────────┐
│ Title                        Status    Updated        │
│ Console Redesign             Published  2h ago       │
│ WASM v2 Benchmarks           Published  1d ago       │
│ 멀티사이트 가이드              Draft      3d ago       │
│ SSG Pipeline Evolution       Ready      5d ago       │
└──────────────────────────────────────────────────────┘

Quick Actions
[✏ New Post] [📦 Install Extension] [⚡ Build & Deploy]
```

**통계 카드**: `stat-card` — light surface bg, border, label/value/change 3줄.
**Recent Posts**: `/api/console/s/{slug}/blog/posts?limit=5` (또는 통합 search API).
**Quick Actions**: 각각 `/s/{slug}/content`, `/s/{slug}/extensions`, `/s/{slug}/deploy`로 라우팅.

### 4.2 ContentPage

전체 콘텐츠를 **extension tabs**로 통합.

```
┌─ Page Header ────────────────┐
│ Content                       │
│ Manage all content across     │
│ extensions                    │
└──────────────────────────────┘

┌──────────────────────────────────────────────────────────────┐
│ Blog(12) Projects(5) Links(8) Movies(15) Books(7) Novels(3) Scraps(24) │
└──────────────────────────────────────────────────────────────┘

Toolbar: [🔍 Search...] [▼ Status] [▼ Language]             [+ New Post]

Table:
┌──────────────────────────────────────────────────────────────┐
│ Title                          Status     Lang   Updated     ⋯│
│ Console Redesign               Published  en     2h ago     ⋯│
│ WASM v2 Benchmarks             Published  en     1d ago     ⋯│
│ 멀티사이트 가이드                Draft      ko     3d ago     ⋯│
└──────────────────────────────────────────────────────────────┘
```

**Extension-specific columns**:

| Extension | Columns |
|-----------|---------|
| Blog | Title, Status (pub/draft), Lang, Updated |
| Projects | Title, Status (active/wip/done), Tech stack, Featured★, Updated |
| Links | Title, URL, Featured★, Order, Updated |
| Movies | Title (year), Rating (stars), Series, Watched date |
| Books | Title, Author, Rating, Read date |
| Novels | Title, Chapters, Total chars, Updated |
| Scraps | Title, Source (HN/GeekNews), Status (queued/published), Collected |

**공통 기능**:
- Search box (client-side filter or server query)
- Status filter dropdown
- "New" button per extension → create form (inline or dedicated page)
- Row click → edit page
- Row actions menu (⋯) → Edit, Publish, Delete

**Empty state**: extension-specific illustration + "No posts yet. Create your first one."

### 4.3 ExtensionsPage

```
┌─ Page Header ──────────────────────────┐
│ Extensions                              │
│ Manage installed extensions for selfhost│
│                     [+ Install Extension]│
└─────────────────────────────────────────┘

INSTALLED (8)
┌──────────────┐ ┌──────────────┐
│ [B] Blog     │ │ [P] Projects │
│ oxibuilder-...  │ │ oxibuilder-...  │
│     [Disable]│ │     [Disable]│
└──────────────┘ └──────────────┘
┌──────────────┐ ┌──────────────┐
│ [L] Links    │ │ [M] Movies   │
│ oxibuilder-...  │ │ oxibuilder-...  │
│     [Disable]│ │     [Disable]│
└──────────────┘ └──────────────┘
... (2-col grid)

AVAILABLE FROM REGISTRY
┌──────────────┐ ┌──────────────┐
│ [A] Activity │ │ [P] Profile  │
│ oxibuilder-...  │ │ oxibuilder-...  │
│    [Install] │ │    [Install] │
└──────────────┘ └──────────────┘
```

- **2-column grid**. 각 카드: extension initial icon + name + crate ID + action button
- **Installed**: "Disable" (destructive outline button) → confirm dialog
- **Available**: "Install" (ghost button) → install flow
- **Disabled extension**: opacity 0.5, grayed out
- **API**: `GET /api/console/s/{slug}/extensions` (list), `PUT .../enable`, `PUT .../disable`

### 4.4 ThemesPage

```
┌─ Page Header ────────────────┐
│ Themes                        │
│ Pick a visual theme for the   │
│ public site                   │
└───────────────────────────────┘

┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐
│ [preview]│ │ [preview]│ │ [preview]│ │ [preview]│
│ Paper    │ │ Midnight │ │ Sepia    │ │ Forest   │
│ ✓ Current│ │          │ │          │ │          │
└──────────┘ └──────────┘ └──────────┘ └──────────┘

Preview — selfhost landing page
┌──────────────────────────────────────┐
│ [mini rendering of public site]      │
│ Lobby with card grid preview         │
└──────────────────────────────────────┘
                              [Apply Theme]
```

- **4-column grid** (responsive: 2-column on small screens)
- 각 card: OKLCH color preview strip + theme name + current checkmark
- **Live preview**: 선택한 테마로 로비를 mini-render (canvas-attached div, 실제 CSS variable 적용)
- **Apply**: `PUT /api/console/s/{slug}/theme` → 성공 시 "✓ Current" 배지 이동
- **API**: 기존 `siteScopedFetch(slug, "/theme")` 패턴

### 4.5 DeployPage

```
┌─ Page Header ────────────────────┐
│ Deploy                            │
│ Build static site and deploy      │
│                   [⇧ Build & Deploy]│
└───────────────────────────────────┘

Last Build — #42  [✓ Deployed]
  ● Build (12s) → ● Generate (3s) → ● Deploy (2s)
  ┌────────────────────────────────┐
  │ [build log lines...]           │
  │ ✓ blog — 12 posts → 12 pages   │
  │ ✓ projects — 5 → 5 pages       │
  │ BUILD COMPLETE — 148 files     │
  └────────────────────────────────┘

Build History
┌───────────────────────────────────────────────┐
│ Build   Duration   Status     Time             │
│ #42     17s        Deployed   2h ago          │
│ #41     15s        Deployed   1d ago          │
│ #40     14s        Deployed   2d ago          │
└───────────────────────────────────────────────┘
```

- **Build trigger**: `POST /api/console/s/{slug}/build` (async, return job ID)
- **Build status poll**: TanStack Query `refetchInterval` polling job status
- **Build log**: monospace dark box, scrollable. API returns log lines or WebSocket stream
- **Build history**: GET build list → table
- **Deploy**: `POST /api/console/s/{slug}/deploy` → status polling

### 4.6 SettingsPage

```
┌─ Page Header ──────┐
│ Settings            │
│ Site-wide config    │
└─────────────────────┘

General
┌──────────────────────────────────────┐
│ Site Title  [My Personal Blog      ] │
│ Base URL    [https://blog.example..] │
│ Language    [Korean (ko)        ▾  ] │
│ Data Dir    [/Users/oxi/.../sites/ ] │ (read-only)
└──────────────────────────────────────┘

Display
┌──────────────────────────────────────┐
│ Default Mode [Grid (card)       ▾  ] │
│ Profile      [Developer         ▾  ] │
└──────────────────────────────────────┘

API Tokens
┌──────────────────────────────────────┐
│ TMDB   [••••••••••••••••          ] │
│ Aladin [not set                   ] │ (disabled if unset)
│ GitHub [oxi                       ] │
└──────────────────────────────────────┘

Danger Zone ▸
┌──────────────────────────────────────┐
│                     [Purge All Data] │
│                     [Delete Site   ] │
└──────────────────────────────────────┘

                              [Reset] [Save Changes]
```

- **Grouped settings cards**: border, rounded, section title as heading
- **Form inputs**: consistent width, inline label layout
- **API Tokens**: masked by default, reveal toggle. 미설정 시 disabled + "not set"
- **Danger Zone**: red-tinted border, destructive outline buttons, confirm dialogs
- **Save**: `PUT /api/console/s/{slug}/config` → dirty state tracking, show "unsaved changes" indicator
- **API**: site config API TBD — v1에서는 `oxibuilder.toml`을 직접 읽는 엔드포인트 필요

## 5. 컴포넌트 구조

### 5.1 신규 컴포넌트

```
web/src/admin/
├── shell/
│   ├── ConsoleShell.tsx        # topbar + sidebar + <Outlet/> (renamed from SiteShell)
│   ├── SiteSelector.tsx        # dropdown + site list panel
│   ├── Sidebar.tsx             # nav items + footer
│   └── Topbar.tsx              # logo + selector + actions
├── dashboard/
│   └── DashboardPage.tsx       # stats + recent + quick actions
├── content/
│   ├── ContentPage.tsx         # tabs + toolbar + active tab content
│   ├── BlogTab.tsx
│   ├── ProjectsTab.tsx
│   ├── LinksTab.tsx
│   ├── MoviesTab.tsx
│   ├── BooksTab.tsx
│   ├── NovelsTab.tsx
│   └── ScrapsTab.tsx
├── extensions/
│   └── ExtensionsPage.tsx      # installed grid + registry section
├── themes/
│   └── ThemesPage.tsx          # catalog cards + preview
├── deploy/
│   └── DeployPage.tsx          # build trigger + log + history
├── settings/
│   └── SettingsPage.tsx        # grouped form cards
└── shared/
    ├── api.ts                  # 확장: 각 extension CRUD endpoints
    ├── stat-card.tsx           # 재사용 stat card
    └── content-table.tsx       # 공통 table (sortable columns, row click, actions menu)
```

### 5.2 재사용할 기존 UI 컴포넌트

| Component | Source | Note |
|-----------|--------|------|
| `Button` | `shared/ui/button.tsx` | variants: primary, ghost, destructive |
| `Badge` | `shared/ui/badge.tsx` | Published/Draft/WIP status |
| `Card` | `shared/ui/card.tsx` | Extension cards, settings sections |
| `EmptyState` | `shared/ui/empty-state.tsx` | Zero-state for every page |
| `Skeleton` | `shared/ui/skeleton.tsx` | Loading placeholders |
| `Tabs` | `shared/ui/tabs.tsx` | Content page extension tabs |
| `DropdownMenu` | `shared/ui/dropdown-menu.tsx` | Site selector, row actions |
| `Input` | `shared/ui/input.tsx` | Search, form fields |
| `Container` | `shared/ui/container.tsx` | Or replaced by sidebar layout |
| `Tooltip` | `shared/ui/tooltip.tsx` | Icon button labels |
| `cn` | `shared/ui/cn.ts` | Conditional classnames |

### 5.3 Sidebar 전용 CSS

`tokens.css`에 console 전용 토큰 추가:

```css
/* Console sidebar — dark, not theme-toggled */
:root {
  --console-sidebar-bg: #1a1e24;
  --console-sidebar-text: #9ca3af;
  --console-sidebar-text-active: #4ade80;
  --console-sidebar-border-active: #22c55e;
  --console-sidebar-hover-bg: rgba(255, 255, 255, 0.04);
  --console-sidebar-label: #6b7280;
}
```

Sidebar는 `data-theme` toggle에 영향받지 않음 — 항상 dark. Main content만 light/dark toggle.

## 6. 상태 관리

### 6.1 TanStack Query keys

```
["sites"]                          → 사이트 목록 (global, site selector)
["site", slug, "stats"]            → 대시보드 통계
["site", slug, "content", ext]     → extension별 콘텐츠 목록
["site", slug, "extensions"]       → 설치된 확장 목록
["site", slug, "theme"]            → 현재 테마
["site", slug, "builds"]           → 빌드 이력
["site", slug, "config"]           → 사이트 설정
```

### 6.2 Site context

`useParams().slug`에서 사이트 식별. ConsoleShell이 `slug`를 context로 제공하거나, 각 페이지가 params에서 직접 읽는다. Context 불필요 — `slug`는 route param으로 충분.

### 6.3 Theme toggle

기존 `ThemeToggle` 유지. Sidebar는 dark 고정, main content만 light/dark 전환.
`document.documentElement.dataset.theme`는 main content CSS variables만 제어.

## 7. API 확장

### 7.1 필요한 새 엔드포인트

| Method | Path | 설명 |
|--------|------|------|
| GET | `/api/console/s/{slug}/stats` | 대시보드 통계 (ext count, post count, storage, uptime) |
| GET | `/api/console/s/{slug}/content/recent` | 최근 게시물 (cross-extension, 최근 5건) |
| GET | `/api/console/s/{slug}/extensions` | 설치된 확장 + 상태 |
| PUT | `/api/console/s/{slug}/extensions/{id}/enable` | 확장 활성화 |
| PUT | `/api/console/s/{slug}/extensions/{id}/disable` | 확장 비활성화 |
| PUT | `/api/console/s/{slug}/theme` | 테마 변경 `{ "theme_id": "paper" }` |
| GET | `/api/console/s/{slug}/theme` | 현재 테마 |
| POST | `/api/console/s/{slug}/build` | 빌드 트리거 → `{ "job_id": "..." }` |
| GET | `/api/console/s/{slug}/build/{id}` | 빌드 상태/로그 |
| GET | `/api/console/s/{slug}/builds` | 빌드 이력 |
| POST | `/api/console/s/{slug}/deploy` | 배포 트리거 |
| GET | `/api/console/s/{slug}/config` | 사이트 설정 |
| PUT | `/api/console/s/{slug}/config` | 사이트 설정 저장 |

### 7.2 기존 엔드포인트 활용

| Method | Path | 용도 |
|--------|------|------|
| GET | `/api/console/sites` | 사이트 목록 (site selector) |
| GET/PUT | `/api/console/sites/default` | 기본 사이트 |
| DELETE | `/api/console/sites/{slug}` | 사이트 삭제 |
| GET | `/api/console/s/{slug}/blog/posts` | Blog content list |
| GET | `/api/console/s/{slug}/projects` | Projects content list |

각 extension의 기존 REST API를 content tabs에서 그대로 소비.

## 8. 반응형

- **Desktop (≥1024px)**: sidebar 200px + main flex-1. Theme grid 4-col, ext grid 2-col.
- **Tablet (768–1023px)**: sidebar 56px (icons only, tooltip), main flex-1.
- **Mobile (<768px)**: sidebar → bottom tab bar. Topbar → hamburger + site selector.

v1은 **desktop only** (콘솔은 데스크톱 브라우저에서 사용한다고 가정). Tablet/Mobile은 v2.

## 9. 빈 상태 / 로딩 / 에러

- **Empty list**: `EmptyState` with extension-specific illustration + CTA button
- **Loading**: `Skeleton` cards/tables matching layout density
- **Error**: inline error card with retry button. `useQuery`의 `isError` + `refetch`
- **API unavailable** (offline): topbar에 "서버 연결 안 됨" indicator. polling 중단.

## 10. 애니메이션

- Sidebar active indicator transition: `transition-all duration-120`
- Page 전환: no animation (콘솔은 정적). `Outlet` swap만
- Site selector panel: `animate-in fade-in slide-in-from-top-1 duration-150`
- Build log: no animation, monospace rendering
- Stat cards: no animation

## 11. 구현 순서

```
Phase 1: Shell
  ├─ ConsoleShell (topbar + sidebar layout)
  ├─ SiteSelector dropdown
  ├─ Sidebar nav
  ├─ Topbar actions
  └─ Route restructuring

Phase 2: Dashboard
  ├─ Stat cards
  ├─ Recent posts table
  ├─ Quick actions
  └─ API 연결 (stats + recent)

Phase 3: Content tabs
  ├─ ContentPage + tab structure
  ├─ BlogTab (list + create/edit placeholder)
  ├─ ProjectsTab
  ├─ MoviesTab
  ├─ BooksTab
  ├─ NovelsTab
  ├─ LinksTab
  ├─ ScrapsTab
  └─ 공통 content-table component

Phase 4: Extensions + Themes
  ├─ ExtensionsPage (grid + toggle)
  ├─ ThemesPage (catalog + preview + apply)
  └─ API 연결

Phase 5: Deploy + Settings
  ├─ DeployPage (build + history)
  ├─ SettingsPage (form groups)
  └─ API 연결

Phase 6: Polish
  ├─ Empty states
  ├─ Error handling
  ├─ Loading skeletons
  └─ Smoke test
```

## 12. 기존 코드 처리

| 파일 | 처리 |
|------|------|
| `web/src/admin/shell/SiteShell.tsx` | `ConsoleShell.tsx`로 대체 |
| `web/src/admin/App.tsx` | 라우트 구조 재작성 |
| `web/src/admin/sites/HomeRedirect.tsx` | 유지 (로직 동일) |
| `web/src/admin/sites/SitesPage.tsx` | 유지 (사이트 관리 페이지) |
| `web/src/admin/sites/NewSiteWizardPage.tsx` | 유지 (사이트 생성 위저드) |
| `web/src/admin/shared/api.ts` | 확장 (새 엔드포인트 추가) |
| `web/src/admin/dashboard/` | `DashboardPage.tsx` 구현 |
| `web/src/admin/content/` | 새 페이지 구현 |
| `web/src/admin/extensions/` | 새 페이지 구현 |
| `web/src/admin/themes/` | 새 페이지 구현 |
| `web/src/admin/deploy/` | 새 페이지 구현 |
| `web/src/shared/tokens.css` | console 전용 토큰 추가 |

## 13. v1 범위 밖

- Tablet/mobile responsive layout
- 실시간 WebSocket build log streaming
- 테마 커스텀 에디터 (OKLCH GUI)
- 확장 마켓플레이스 GUI
- 멀티 유저 인증
- 백업 스케줄링 UI
