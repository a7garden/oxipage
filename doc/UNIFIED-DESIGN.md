> Canonical unified design system: project-oxi/.github/DESIGN.md — this file is project-specific.

# oxipage — Unified Design System (Project Adaptation)

> **정규 문서:** `oxinot/doc/UNIFIED-DESIGN.md` (oxi 생태계 통합 디자인 시스템 v1.0).
> 이 파일은 oxipage가 그 위에 갖는 **고유 표면 정체성 + 마이그레이션 경로**만 다룬다. 공통 토큰·컴포넌트·철학은 정규 문서를 따른다.
>
> **버전:** v1.0 · **작성일:** 2026-07-31

---

## oxipage의 정체성

oxipage는 한 사람의 여러 자아(개발자·소설가·비평가·큐레이터)를 모으는 다중 확장 개인 사이트다. "밤에 코드를 짜다가 문득 다음 문장을 이어 쓰는 조용한 작업실"이 컨셉이다. 공통 문법(ink on paper, 6-hue OKLCH)을 따르되, **로비 3모드와 별점 골드**가 oxipage만의 시그니처다.

---

## 공통 시스템에서 변경 없는 것

아래는 정규 문서(§1–10)를 그대로 따른다 — oxipage 고유값이 아님:

- OKLCH 3-tier 토큰 (primitive → semantic → component)
- 중성 warm paper / cool ink 램프 (hue 95 light / 265 dark) — oxipage v1과 동일
- 6-hue 라벨 팔레트 (L≈0.70–0.75, C≈0.12–0.15)
- 상태색 (APCA 최적화)
- SUIT(본문) + SUITE(헤드라인) + Geist Mono(코드)
- 4px 스페이싱, 반경 스케일, 4단계 elevation
- `.dark` 클래스 단일 트리거

---

## oxipage 고유값 (공통 시스템 위에 유지)

### 1. 로비 3모드 (`LobbyConfig.display_mode`)

| 모드 | 설명 | 정규 문서 |
|---|---|---|
| `list` | 세로 행 스택, 헤어라인 구분선, 모션 없음. 정보 밀도 최고. reduced-motion 시 canvas의 폴백 | §4.3, §9.2 |
| `grid` | 반응형 카드 그리드 (모바일 1열 → 데스크톱 3~4열). 호버 시 절제된 리프트 | §4.3 |
| `canvas` | 카드가 넓은 뷰포트에 흩어져 느린 앰비언트 드리프트. **사이트의 시그니처 과격 지점**. reduced-motion → 자동 `grid` 폴백 | §9.2 |

```ts
// canvas 기본값
type CanvasParams = {
  drift_amplitude_px: number;   // 12
  drift_period_s:     number;   // 14
  seed:               string;   // "stable-per-day"
};
```

초기 배치는 단순 충돌 회피 패스(전체 물리 시뮬 아님). 드리프트는 CSS `transform` 키프레임 — JS rAF 아님.

### 2. 별점 골드 (oxipage 전용, 유지)

별점 전용 "잉크에 찍은 금박" 톤. 6-hue 라벨과 분리된 독립 토큰.

```css
/* primitives — oxipage 로컬 */
--p-gold-400: oklch(82% 0.15 85);
--p-gold-500: oklch(78% 0.15 85);
--p-gold-600: oklch(68% 0.15 85);

/* semantic */
[data-theme="light"] { --color-rating-fill: var(--p-gold-600); }
[data-theme="dark"]  { --color-rating-fill: var(--p-gold-500); }
```

마이그레이션 후: `--color-rating-fill` 토큰을 유지하되 트리거를 `[data-theme]` → `.dark`로 전환.

### 3. 퍼블릭 테마 축 (6테마 — 독립 변형 축)

oxipage는 공개 사이트에 6가지 퍼블릭 테마(`paper`/`midnight`/`sepia`/`forest`/`neon`/`canvas`)를 제공한다. 이는 `.dark` light/dark 축과 **직교**하는 독립 변형 축이다 (정규 문서 §8.4와 동일 원리).

```css
[data-public-theme="paper"],
[data-public-theme="midnight"],
[data-public-theme="sepia"],
[data-public-theme="forest"],
[data-public-theme="neon"],
[data-public-theme="canvas"] {
  --public-accent-400: oklch(78% 0.12 var(--accent-hue));
  --public-accent-500: oklch(62% 0.14 var(--accent-hue));
  --public-accent-600: oklch(50% 0.14 var(--accent-hue));
  --public-accent-700: oklch(40% 0.12 var(--accent-hue));
}
```

`[data-public-theme]`는 콘솔 `data-theme`과 독립 — `DraftPreviewPane` 내 퍼블릭 테마 교환이 Admin 셸을 다시 칠하지 않는다.

---

## 마이그레이션 경로 (v1 → unified)

현재 `web/src/shared/tokens.css`는 v1 상태. 우선순위:

### Step 1: 토큰 재구조
- `[data-theme="light"]` → `:root`
- `[data-theme="dark"]` → `.dark`
- pine green 악센트(`--p-accent-*` hue 160) → 6-hue 라벨 팔레트 + `--color-interactive-primary`(blue, 정규 §2.3)
- tier 분리: `tokens/primitives.css` / `tokens/semantic.css` / `tokens/semantic-dark.css` / `tokens/components.css` / `tokens/theme.css`

### Step 2: 폰트 마이그레이션
- Pretendard → **SUIT** (`--font-body` → `--font-sans`)
- Fraunces → **SUITE** (`--font-display` — 세리프 제거, 정규 §3.1)
- **line-height 재보정 필수:** Pretendard x-height이 SUIT보다 ~2% 큼. 본문 1.55→1.50, 소형 1.5→1.45. visual diff 확인.
- jsDelivr에서 로드: `@import url('https://cdn.jsdelivr.net/gh/sun-typeface/SUIT@2/...')`

### Step 3: 반경 정렬
- oxipage v1: `--radius-md: 0.75rem` (12px). 정규 시스템: `--radius-md: 0.5rem` (8px).
- 카드는 `--radius-lg`(12px)로 정렬. 버튼/인풋은 `--radius-md`(8px). 컴포넌트별 토큰(`--card-radius` 등) 도입으로 일원화.

### Step 4: FOUC 스크립트
- `web/public/theme-boot.js` → 정규 §8.2 인라인 `<head>` 스크립트로 교체. 저장 키 `oxi-theme`.

### Step 5: 콘솔 사이드바 레거시
- 레거시 그린(`#22c55e`, `#4ade80`): **v1 유지**. v2에서 `bg-status-success`로 교체. 레거시 호환을 위해 `.dark` → `[data-theme="dark"]` 미러링 MutationObserver를 전환기에 유지(차기 마이너에서 제거).

### Step 6: 컴포넌트 스윕
- `web/src/**/*.{tsx,ts}`에서 모든 `dark:` 제거 → 시맨틱 유틸리티로.
- `data-theme="dark"` 리스너 → `class="dark"` MutationObserver.

---

## 검증 체크리스트

- [ ] 로비 3모드 smoke test (list/grid/canvas 전환)
- [ ] canvas reduced-motion → grid 폴백 확인
- [ ] 본문/헤딩 APCA 대비 (Lc ≥ 60 본문, Lc ≥ 75 캡션)
- [ ] Pretendard→SUIT visual diff (line-height 재보정 후)
- [ ] 별점 골드 토큰 표시 정상 (양 테마)
- [ ] `[data-public-theme]` 6테마 독립 동작

---

*공통 토큰 값·컴포넌트 스펙·철학은 `oxinot/doc/UNIFIED-DESIGN.md`를 따른다. 이 파일은 oxipage 고유 영역만.*
