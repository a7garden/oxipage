# Oxipage v2 — Static Site Generator

> 설계 문서. 2026-07-28.

## 1. 동기

Oxipage v1은 상시 가동 서버(`oxipage-core`)에 의존한다. 블로그·포트폴리오는 콘텐츠가 발행 시점에 결정되고 방문자는 항상 최신 스냅샷을 본다. 정적 사이트 생성기로 전환하면:

- **런타임 의존성 제거** — GitHub Pages, Cloudflare Pages, Netlify 등 어디든 배포 가능
- **운영 비용 제로** — 서버 프로세스, DB 커넥션, 백업 부담 없음
- **보안 표면 축소** — 공개 사이트에는 API도 DB도 없음
- **에이전트 경험 보존** — CLI는 로컬 SQLite로 그대로 동작

핵심: **관리는 동적으로, 배포는 정적으로.**

## 2. 아키텍처

```
┌──────────────────────────────────────────────────────┐
│                    로컬 (Mac mini)                     │
│                                                      │
│  oxipage serve       oxipage blog new ...            │
│  (admin-web)         oxipage query "SELECT ..."      │
│       │                     │                        │
│       └───────┬─────────────┘                        │
│               ▼                                       │
│          SQLite DB                                   │
│               │                                       │
│               ▼                                       │
│        oxipage build                                 │
│               │                                       │
│               ▼                                       │
│          out/ (정적 파일)                              │
│               │                                       │
└───────────────│───────────────────────────────────────┘
                │  oxipage deploy
                ▼
┌──────────────────────────────────────┐
│   GitHub Pages / Cloudflare Pages     │
│   순수 정적 파일 서빙, 서버 없음        │
└──────────────────────────────────────┘
```

### 2.1 아키텍처 변경점

| 계층 | v1 | v2 |
|---|---|---|
| 콘텐츠 저장소 | SQLite | SQLite (불변) |
| 관리 API | oxipage-core 상시 서버 | `oxipage serve` 로컬 서버 (불변) |
| 관리 UI | admin-web SPA → API | admin-web SPA → 로컬 API (불변) |
| CLI | HTTP 클라이언트 | HTTP 클라이언트 + 직접 SQLite 접근 |
| 공개 사이트 | oxipage-core가 SSR + SPA 서빙 | 정적 HTML + JSON + JS |
| 데이터 페칭 | TanStack Query → REST API | TanStack Query → 정적 JSON 파일 |
| 검색 | 서버 FTS5 API | FTS5로 빌드 시 인덱싱 → 정적 JSON → 클라이언트 매칭 |
| OG/SEO | 서버가 publish 시 스냅샷 생성 | 빌드 시 모든 페이지에 OG 태그 박제 |
| 배포 | 직접 서버 실행 | `oxipage deploy` (GitHub Pages 1순위) |

### 2.2 변경되지 않는 것

- Extension 트레이트 구조 (기존 `Extension`은 유지, `BuildExt` 추가)
- CLI 서브커맨드 (`blog`, `project`, `link`, `lobby` 등 전부 보존)
- admin-web (`oxipage serve`로 그대로 동작)
- 데이터 모델 (모든 테이블 스키마 불변)
- WASM 확장 런타임 (관리 모드에서만 동작)
- 디자인 토큰 / 다크모드 / 로비 레이아웃

## 3. Extension 트레이트 분화

### 3.1 기존 트레이트 (불변)

```rust
#[async_trait]
pub trait Extension: Send + Sync {
    fn id(&self) -> &'static str;
    fn display_name(&self, lang: Lang) -> String;
    fn migrations(&self) -> Vec<Migration>;
    fn routes(&self) -> Router<AppState>;
    fn cli(&self) -> Option<clap::Command>;
    fn background_jobs(&self) -> Vec<Box<dyn ScheduledJob>>;
    async fn lobby_summary(&self, ctx: &AppState) -> Option<LobbyCard>;
}
```

### 3.2 빌드 트레이트 (신규)

빌드는 CPU 바운드 작업이므로 `async`를 사용하지 않는다. 각 확장은 독립적이므로 `rayon`으로 병렬 처리한다.

```rust
use rayon::prelude::*;

/// 각 확장이 정적 사이트 빌드를 위해 구현해야 하는 트레이트.
/// 모든 메서드는 동기(sync). 병렬 실행 가능.
pub trait BuildExt {
    type Error: std::error::Error + Send + 'static;

    /// 정적 HTML 페이지 생성. DB의 published 콘텐츠만 읽는다.
    /// 반환: (URL 경로, HTML 문자열) 벡터
    fn build_pages(&self, db: &SqlitePool) -> Result<Vec<StaticPage>, Self::Error>;

    /// 클라이언트 사이드 데이터. React SPA가 fetch할 정적 JSON.
    fn build_data(&self, db: &SqlitePool) -> Result<Box<dyn erased_serde::Serialize>, Self::Error>;

    /// 검색 인덱스 문서. 코어가 취합해 search-index.json으로 덤프.
    fn build_search_docs(&self, db: &SqlitePool) -> Result<Vec<SearchDoc>, Self::Error>;
}
```

```rust
/// 빌드 파이프라인 — rayon으로 각 확장을 병렬 처리
fn build_all(extensions: &[Box<dyn BuildExt>], db: &SqlitePool) -> Result<BuildOutput> {
    let results: Vec<_> = extensions
        .par_iter()
        .map(|ext| {
            let pages = ext.build_pages(db)?;
            let data = ext.build_data(db)?;
            let search_docs = ext.build_search_docs(db)?;
            Ok(ExtBuildOutput { pages, data, search_docs })
        })
        .collect::<Result<Vec<_>>>()?;

    // 결과 병합
    BuildOutput::merge(results)
}
```

### 3.3 빌드 파이프라인

```
oxipage build [--site <name>]

1. DB 연결 (읽기 전용)
2. 각 활성화된 확장에서 build_pages()  → out/<ext>/**/*.html
3. 각 활성화된 확장에서 build_data()   → out/data/<ext>.json
4. 각 활성화된 확장에서 build_search_docs() → 취합 → out/data/search-index.json
5. 공통 정적 자산 복사
   - web/dist/ (React SPA 번들)      → out/
   - /data/media/ (이미지)           → out/media/
   - 마크다운 원본 덤프 (blog, novels only) → out/<ext>/<slug>/index.md
6. 로비 페이지 생성 (매니페스트 기반)
```

## 4. 마크다운 원본 동반 출력

블로그 본문과 소설 챕터처럼 **원본이 마크다운인 콘텐츠**에만 `.md` 파일을 동반 출력한다.
영화·책·프로젝트 등 구조화 데이터는 JSON이 더 정확하고 기계 친화적이므로 `.md` 변환을 하지 않는다.

```
out/blog/rust-static-blog/
├── index.html          # 정적 HTML (OG 메타 포함)
├── index.md            # 원본 마크다운 (DB body 필드 복사)
└── index.json          # 구조화 메타데이터 (제목, 날짜, 태그)

out/novels/my-story/chapter-1/
├── index.html
├── index.md            # 챕터 본문 원본
└── index.json
```

**원칙:** 원본이 마크다운이면 `.md`도 내보낸다. 예외 없음.

## 5. 검색

### 5.1 빌드 시점: FTS5 인덱싱

`oxipage build`가 각 확장의 `build_search_docs()`를 호출해 문서 벡터를 수집하고,
SQLite FTS5 trigram으로 인덱싱한 후 결과를 정적 JSON으로 덤프한다.

### 5.2 인덱스 구조

```json
[
  {
    "id": "blog/my-post",
    "title": "Rust로 만든 정적 블로그",
    "body_preview": "Oxipage v2는 정적 사이트 생성기로...",
    "type": "blog",
    "url": "/blog/my-post",
    "published_at": "2026-07-20"
  }
]
```

- 1,000문서 기준 인덱스 크기: ~200KB
- 클라이언트가 전체 로드 후 `title + body_preview`에서 `String.includes()`로 필터링
- 1,000문서 × 500자 = 500KB, 브라우저 `includes()`는 수 ms
- 한국어 부분 문자열 매칭은 빌드 타임에 FTS5 trigram이 처리

## 6. 미디어 파이프라인

| 시점 | 작업 |
|---|---|
| **업로드 (관리)** | 원본 저장 + 썸네일 생성 (현재와 동일) |
| **`oxipage build`** | `/data/media/` → `out/media/` 복사만 |
| **`oxipage build --optimize`** | 추가로 webp 변환, srcset 생성 |

이미지 처리 실패로 빌드가 깨지지 않도록 **최적화는 별도 플래그**로 분리. 기본 빌드는 복사만 수행.

## 7. 배포

```
oxipage deploy [--target <target>] [--site <name>]
```

| 타겟 | 우선순위 | 방식 |
|---|---|---|
| GitHub Pages | **1순위** | `gh-pages` 브랜치에 `out/` force-push |
| Cloudflare Pages | 2순위 | Wrangler Pages deploy API |
| Netlify | 3순위 | Netlify CLI deploy |

### 7.1 GitHub Pages 배포 흐름

```
1. gh auth status 확인
2. git worktree add /tmp/oxipage-deploy gh-pages
3. cp -r out/* → worktree
4. git commit -m "deploy: $(date -Iseconds)"
5. git push origin gh-pages
6. git worktree remove /tmp/oxipage-deploy
```

## 8. 프리뷰

```
oxipage serve --preview    # out/ 디렉토리를 로컬 HTTP 서빙
```

`file://` 프로토콜은 CORS와 ES 모듈 로딩을 깨므로 반드시 HTTP 서버가 필요하다.
`oxipage serve`에 `--mode preview`를 추가해 `admin-web` 대신 `out/`을 정적 서빙.

## 9. 외부 API / 캐시 갱신

`oxipage build`는 외부 API를 호출하지 않는다. 빌드는 순수 DB → 정적 파일 변환만.
외부 데이터 수집은 별도 명령으로 분리:

```
oxipage cache refresh [--extension <name>]
  ├── activity: GitHub Events API 수집
  ├── movies:   TMDB 포스터 URL 갱신
  ├── books:    알라딘/Google Books 메타데이터 갱신
  └── scraps:   HN/GeekNews 최신 글 수집
```

관심사 분리: `cache refresh` → DB 기록 → `build`가 읽는다. 빌드는 예측 가능해야 한다.

## 10. React SPA 데이터 레이어

| 현재 | 변경 후 |
|---|---|
| `fetch('/api/v1/blog/posts')` | `fetch('/data/blog.json')` |
| `fetch('/api/v1/projects')` | `fetch('/data/projects.json')` |
| `fetch('/api/v1/search?q=rust')` | `fetch('/data/search-index.json')` + 클라이언트 필터링 |

변경은 TanStack Query의 `queryFn`만 바꾸면 된다. 컴포넌트, 라우팅, 디자인 토큰 전부 불변.

Vite 환경 변수 `VITE_DATA_MODE`로 데이터 소스를 분기:

```
개발 모드:   fetch → localhost:8787/api/v1/...     (지금과 동일)
프리뷰 모드: fetch → localhost:8787/data/...        (정적 JSON)
프로덕션:    fetch → /data/...                       (배포된 정적 JSON)
```

## 11. 증분 빌드

```
oxipage build              # 전체 빌드
oxipage build --incremental  # 변경된 콘텐츠만 (v2.1에서)
```

`blog_posts.updated_at` 컬럼과 마지막 빌드 타임스탬프를 비교.
**v2 초기에는 전체 빌드만 지원** (YAGNI).

## 12. 멀티사이트

기존 `sites.toml`을 그대로 활용:

```toml
[site.blog]
base_url = "https://blog.a7garden.dev"
extensions = ["blog", "profile", "links", "novels"]

[site.portfolio]
base_url = "https://a7garden.dev"
extensions = ["profile", "projects", "activity", "movies", "books", "scraps"]
```

```
oxipage build --site blog        → out/blog/
oxipage build --site portfolio   → out/portfolio/
oxipage deploy --site blog       → blog.a7garden.dev
```

`oxipage build`(site 생략)는 기본 사이트를 빌드.

## 13. CLI 변경점

### 13.1 SQL 쿼리 (신규)

```
oxipage query "<SQL>" [--json]
```

- CLI 바이너리가 `oxipage.db`를 직접 열어(`sqlx`) 쿼리 실행
- **읽기 전용** — INSERT/UPDATE/DROP은 차단
- 결과는 `--json` 플래그로 JSON 배열 출력

```bash
$ oxipage query "SELECT slug, title FROM blog_posts WHERE tags LIKE '%rust%'" --json
[{"slug":"rust-static-blog","title":"Rust로 만든 정적 블로그"}]
```

### 13.2 스키마 발견 (신규)

```
oxipage schema [--extension <name>] [--json]
```

```bash
$ oxipage schema --extension blog --json
{
  "tables": [
    {
      "name": "blog_posts",
      "columns": [
        {"name": "id", "type": "INTEGER", "pk": true},
        {"name": "slug", "type": "TEXT", "unique": true},
        {"name": "title", "type": "TEXT"},
        {"name": "body", "type": "TEXT"},
        {"name": "lang", "type": "TEXT"},
        {"name": "tags", "type": "JSON"},
        {"name": "published_at", "type": "TEXT", "nullable": true}
      ]
    }
  ]
}
```

AI 에이전트가 DB 구조를 모르더라도 `oxipage schema --json` 한 번으로 모든 걸 파악 가능.

### 13.3 Diff (v2.1)

```
oxipage blog diff <slug> [--json]
```

낮은 우선순위. DB에 리비전 이력이 없으면 현재 버전만 표시.
에이전트가 `oxipage blog show`로 현재 내용을 읽고 직접 이전/이후를 비교할 수 있으므로 v2 초기에는 없어도 무방.

### 13.4 신규 서브커맨드

```
oxipage build [--site <name>] [--optimize] [--incremental]
oxipage deploy [--target github|cloudflare|netlify] [--site <name>]
oxipage query "<SQL>" [--json]
oxipage schema [--extension <name>] [--json]
oxipage cache refresh [--extension <name>]
oxipage serve --preview
```

## 14. AI 에이전트 친화성

### 14.1 요구사항

| # | 기능 | 우선순위 | 상태 |
|---|---|---|---|
| 1 | `oxipage query --json` | **필수** | 신규 |
| 2 | `oxipage schema --json` | **필수** | 신규 |
| 3 | `oxipage blog diff --json` | 있으면 좋음 | v2.1 |
| 4 | 배치 `blog new --file posts/*.md` | 있으면 좋음 | v2.1 |
| 5 | `oxipage status --json`에 최근 작업 내역 | 있으면 좋음 | v2.1 |

### 14.2 에이전트 워크플로우

```bash
# 1. DB 구조 파악
oxipage schema --json

# 2. 자유 질의
oxipage query "SELECT slug, title FROM blog_posts
  WHERE published_at IS NOT NULL ORDER BY published_at DESC" --json

# 3. 콘텐츠 읽기
oxipage blog show my-post --json

# 4. 편집
cat > /tmp/body.md << 'EOF'
수정된 본문...
EOF
oxipage blog edit my-post --file /tmp/body.md --json

# 5. 빌드 & 배포
oxipage build && oxipage deploy
```

## 15. Rust 활용

| 결정 | 이유 |
|---|---|
| `BuildExt` 메서드는 `async` 미사용 | 빌드는 CPU 바운드. `#[async_trait]` 불필요 |
| `rayon`으로 확장별 빌드 병렬 처리 | 확장 간 의존성 없음, 순차 처리할 이유 없음 |
| `Result<T, E>` 반환 | DB 조회, JSON 직렬화, 파일 쓰기 실패 경로 명시 |
| `erased_serde::Serialize` | 각 확장이 자기 타입을 유지하면서 코어는 동질적 처리 |
| Edition 2024 + sqlx 0.8 + tokio | 최신 안정 버전, 검증된 조합 |

## 16. 구현 범위 요약

| 컴포넌트 | 변경 |
|---|---|
| `oxipage-core` | `BuildExt` 트레이트 추가, 빌드 파이프라인 (rayon 병렬), `cache refresh` 분리 |
| 각 `oxipage-ext-*` | `BuildExt` 구현체 추가 (9개 확장) |
| `oxipage-cli` | `query`, `schema`, `build`, `deploy`, `cache`, `serve --preview` 서브커맨드 추가 |
| `web/` | 데이터 페칭을 정적 JSON으로 전환 (TanStack Query `queryFn` 변경 + `VITE_DATA_MODE`) |
| `admin-web/` | 불변 |
| `oxipage-server` | `serve --preview` 모드 추가 |
| SQLite 스키마 | 불변 |
| 배포 모델 | 상시 서버 → 정적 파일 (`oxipage deploy` → GitHub Pages 1순위) |
