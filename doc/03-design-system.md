# 3장 — 디자인 시스템

## 3.1 방향성

개발자·소설가·비평가·큐레이터 — 한 사람의 여러 자아를 모으는 사이트이니 "밤에 코드를 짜다가 문득 다음 문장을 이어 쓰는 조용한 작업실"이라는 한 문장을 컨셉으로 잡습니다. 요즘 AI가 기본값으로 자주 내놓는 세 가지 룩 — ① 크림색 배경 + 세리프 + 테라코타 악센트, ② 거의 검정 배경 + 형광 그린/버밀리언 단색 악센트, ③ 헤어라인 룰 + 각진 신문 레이아웃 — 은 전부 피하고, 종이와 잉크의 질감에 파인 그린 하나만 악센트로 쓰는 쪽을 제안합니다. 이 절의 실제 값은 **시작점 제안**이며, 최종 취향은 만드는 사람 몫입니다.

**시그니처 요소 제안:** 로비의 `canvas` 모드(§3.4) — 카드가 물 위에 뜬 종이처럼 천천히 떠다니는 것 — 를 이 사이트를 기억하게 만드는 단 하나의 과감한 지점으로 삼고, 나머지 UI(타이포, 카드, 폼)는 최대한 절제합니다.

## 3.2 왜 OKLCH인가

- **지각적 균일성:** 같은 L(명도) 차이가 색상환 어디서든 비슷한 밝기 차이로 보입니다. 다크/라이트 모드를 만들 때 색상(H)·채도(C)는 고정하고 L만 뒤집으면, RGB/HSL에서 흔히 발생하는 "다크모드에서 유독 탁하거나 유독 튀는 색" 문제가 크게 줄어듭니다.
- **대비 계산이 예측 가능:** L 값 자체가 지각 밝기에 가까워서, 텍스트/배경 조합의 대비를 감으로 맞추지 않고 L 차이로 설계할 수 있습니다(WCAG 대비비를 완전히 대체하진 않지만 1차 필터로 유용).
- **최신 CSS(`color: oklch(...)`)에서 네이티브로 지원**되므로 별도 전처리 없이 커스텀 프로퍼티로 바로 씁니다.

## 3.3 토큰 아키텍처 (3단계)

```
Primitive tokens  →  Semantic tokens  →  Component tokens
 (원색 램프)          (역할별 이름)         (버튼/카드 등 개별 컴포넌트)
```

### Primitive (원색 램프)

```css
:root {
  /* 중성색 — "종이/잉크", 아주 낮은 채도, 살짝 따뜻한 톤(95도) */
  --p-neutral-0:   oklch(98.5% 0.004 95);
  --p-neutral-50:  oklch(95%   0.006 95);
  --p-neutral-100: oklch(90%   0.007 95);
  --p-neutral-300: oklch(75%   0.010 95);
  --p-neutral-500: oklch(55%   0.012 95);
  --p-neutral-700: oklch(35%   0.012 265);
  --p-neutral-900: oklch(18%   0.015 265); /* 잉크 */
  --p-neutral-950: oklch(13%   0.020 265); /* 가장 깊은 다크 배경 */

  /* 악센트 — "파인 그린", 절제된 배경과 대비 */
  --p-accent-400: oklch(78% 0.12 160);
  --p-accent-500: oklch(62% 0.14 160);
  --p-accent-600: oklch(50% 0.14 160);
  --p-accent-700: oklch(40% 0.12 160);

  /* 별점 전용 — 악센트와 분리된 "잉크에 찍은 금박" 톤 */
  --p-gold-500: oklch(78% 0.15 85);
  --p-gold-600: oklch(68% 0.15 85);

  /* 상태색 */
  --p-danger-500:  oklch(55% 0.19 25);
  --p-success-500: oklch(60% 0.15 145);
}
```

### Semantic (라이트/다크 매핑)

```css
[data-theme="light"] {
  --color-bg-canvas:      var(--p-neutral-0);
  --color-bg-surface:     var(--p-neutral-50);
  --color-bg-surface-raised: oklch(100% 0 0);
  --color-text-primary:   var(--p-neutral-900);
  --color-text-secondary: var(--p-neutral-700);
  --color-text-tertiary:  var(--p-neutral-500);
  --color-border:         var(--p-neutral-100);
  --color-accent:         var(--p-accent-600);
  --color-accent-contrast:oklch(100% 0 0);
  --color-rating-fill:    var(--p-gold-600);
  --color-danger:         var(--p-danger-500);
  --color-success:        var(--p-success-500);
}

[data-theme="dark"] {
  --color-bg-canvas:      var(--p-neutral-950);
  --color-bg-surface:     var(--p-neutral-900);
  --color-bg-surface-raised: oklch(22% 0.016 265);
  --color-text-primary:   var(--p-neutral-0);
  --color-text-secondary: var(--p-neutral-300);
  --color-text-tertiary:  var(--p-neutral-500);
  --color-border:         oklch(28% 0.015 265);
  --color-accent:         var(--p-accent-400);
  --color-accent-contrast:var(--p-neutral-950);
  --color-rating-fill:    var(--p-gold-500);
  --color-danger:         oklch(65% 0.18 25);
  --color-success:        oklch(68% 0.14 145);
}
```

같은 `--p-accent-*` 램프에서 라이트는 더 어두운 600을(밝은 배경 위 대비), 다크는 더 밝은 400을(어두운 배경 위 대비) 골라 쓰는 것이 핵심 패턴입니다. **H(160), C(~0.12-0.14)는 라이트/다크 어디서도 고정** — 이것이 OKLCH를 쓰는 실질적 이유입니다.

### Component 예시

```css
.card {
  background: var(--color-bg-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md, 0.75rem);
  color: var(--color-text-primary);
}
.rating-star--filled { color: var(--color-rating-fill); }
.btn-primary {
  background: var(--color-accent);
  color: var(--color-accent-contrast);
}
```

## 3.4 다크/라이트 전환 메커니즘

- 기본값은 `prefers-color-scheme` 미디어쿼리를 따르되, 사용자가 명시적으로 고르면 `localStorage`에 저장(이건 실서비스이므로 브라우저 스토리지 사용에 제약 없음).
- FOUC(테마 반짝임) 방지를 위해 `<html>`의 `<head>` 최상단에 렌더 블로킹 인라인 스크립트를 두어, 페인트 전에 `data-theme` 속성을 결정합니다.
- §1.6의 SSR 스냅샷은 크롤러용이라 테마 개념이 없어도 되지만, OG 이미지 등은 라이트 기준으로 고정 생성합니다.

## 3.5 타이포그래피

| 역할 | 용도 | 제안 |
|---|---|---|
| Display | 로비 히어로, 섹션 타이틀 | 개성 있는 세리프 1종, 절제해서 사용(본문에는 절대 안 씀) |
| Body/UI | 본문, 버튼, 네비게이션 (한/영 혼용) | 한글+라틴을 모두 잘 지원하는 가변폭 휴먼 산세리프 1종(예: Pretendard 계열) — KO/EN 전환 시 폰트가 안 바뀌어야 리듬이 깨지지 않음 |
| Mono | 코드 블록, 커밋 활동 피드, 태그 | 고정폭 모노스페이스 1종 |

정확한 폰트 페어링은 취향의 영역이라 강제하지 않지만, **"본문은 한/영 공용 1종, Display는 절제된 포인트로만"** 이라는 원칙만 지키면 이중언어 사이트에서 흔한 "언어 바뀔 때마다 리듬이 깨지는" 문제를 피할 수 있습니다.

## 3.6 로비 레이아웃 3종

`LobbyConfig.display_mode`(2장 §2.12)로 확장별 독립 설정.

### `list` — 정갈

- 세로로 쌓인 행. 각 행: 확장 아이콘 + 이름 + 한 줄 요약 + 최근 항목 3개 미니 리스트
- 헤어라인 구분선, 모션 없음
- 정보 밀도가 가장 높음. `prefers-reduced-motion: reduce`일 때 `canvas` 모드의 자동 폴백 대상

### `grid` — 카드

- 반응형 카드 그리드(모바일 1열 → 데스크톱 3~4열)
- 카드 표지: 확장 특성에 맞는 대표 비주얼(영화는 최근 본 작품 포스터 콜라주, 프로젝트는 스크린샷, 소설은 커버, 링크는 썸네일)
- 호버 시 살짝 떠오르는 정도의 절제된 모션

**`style_params` 예시(grid):**
```json
{ "columns": { "mobile": 1, "tablet": 2, "desktop": 3 }, "cover_style": "collage" }
```

### `canvas` — 플로팅

- 카드가 넓은 뷰포트 안에 흩어져 배치되고, 아주 느린 앰비언트 드리프트(transform keyframe, 물리 엔진까지는 v1 범위 밖)로 살아있는 느낌을 줌
- 배치는 매 방문마다 겹치지 않도록 서버 또는 클라이언트에서 간단한 충돌 회피 패스를 한 번 돌려 좌표를 계산(완전한 물리 시뮬레이션이 아니라 "겹치지 않는 초기 배치 + 느린 흔들림" 정도로 스코프를 제한)
- 카드를 클릭하면 해당 확장으로 이동
- **`prefers-reduced-motion: reduce`이면 자동으로 `grid` 모드로 폴백** — 접근성 원칙상 예외 없음

**`style_params` 예시(canvas):**
```json
{ "drift_amplitude_px": 12, "drift_period_s": 14, "seed": "stable-per-day" }
```

## 3.7 접근성 최소 기준

- 키보드 포커스 링은 `--color-accent`로 항상 시각적으로 드러나야 함(어떤 컴포넌트도 `outline: none`만 하고 대체 스타일 없이 끝내지 않기)
- 본문 텍스트 대비는 라이트/다크 각각 WCAG AA(4.5:1) 이상 — §3.3의 L 값들은 대략적인 시작값이므로 실제 구현 시 대비비 계산 도구로 재검증
- 모션은 전부 `prefers-reduced-motion`을 존중(§3.6 `canvas` 폴백 포함)

---

## 3.8 v2 — 컴포넌트 레이어 (2026-07-27)

v1의 토큰 시스템(§3.2–3.3)과 컨셉(§3.1)은 100% 보존한 채, 그 위에 **헤드리스 컴포넌트 라이브러리**를 올려 비주얼 실현도를 끌어올렸다. 상세 설계는 `docs/superpowers/specs/2026-07-27-design-system-v2-design.md` 참조.

### 스택

- **Radix UI** (a11y 보장, unstyled) + **Tailwind v4** (`@theme inline`) + **class-variance-authority**.
- shadcn CLI를 쓰지 않고 **패턴만 차용**해 직접 작성 — CLI는 자체 토큰을 덮어쓰므로 우리 OKLCH 시스템과 충돌.
- 핵심: `@theme inline`(plain `@theme` 아님). plain은 빌드타임에 값을 베이크하여 §3.4의 `data-theme` 런타임 전환을 깨뜨린다. `inline`은 `var()` 참조를 유지해 토큰 스위칭이 모든 유틸리티에 자동 전파.

### 토큰 노출 (`tokens.css`)

`@theme inline` 블록이 시맨틱 토큰을 Tailwind 유틸리티로 별칭. 별칭 이름은 원본 `--color-*` 프리미티브와 **충돌하지 않게** 설계(자기참조 `var()` 회피):

| 유틸리티 | 참조 | 의미 |
|---|---|---|
| `bg-canvas` / `bg-surface` / `bg-raised` | `--color-bg-*` | 배경 3단 |
| `text-foreground` / `text-muted` / `text-subtle` | `--color-text-primary/secondary/tertiary` | 텍스트 3단 |
| `border-line` | `--color-border` | 보더 |
| `bg-primary` / `text-primary-foreground` | `--color-accent` / `--color-accent-contrast` | 악센트(파인 그린) |
| `text-star` | `--color-rating-fill` | 별점 골드 |
| `font-serif` | `--font-display` (Fraunces) | 디스플레이 세리프 |

### 추가된 토큰

- **Elevation** (`--elevation-xs/sm/md/lg`, `[data-theme]`별 themed): 갭 1(컴포넌트 밋밥) 해결. 라이트는 옅은 잉크 그림자, 다크는 더 깊고 불투명한 그림자.
- **Display 폰트** (`--font-display: Fraunces, Noto Serif KR`): 갭 2. 본문은 Pretendard 유지, 타이틀에만 세리프.
- **Radius/Type/Motion 스케일**, `--text-display` 단계.

### 컴포넌트 카탈로그 (`shared/ui/`)

15개 프리미티브. shadcn v4 패턴(`React.ComponentProps<>` + `data-slot`, forwardRef 제거):
Button(cva 5 variant), Card family, Input/Textarea/Label, Badge(6 variant), Avatar/Separator/Skeleton, Tooltip/Tabs/DropdownMenu(Radix), Container/PageHeader/EmptyState. `RatingStars`는 자체 완결 Tailwind로 리팩터(외부 CSS 의존 제거).

### 적용 범위

App shell(스티키 반투명 헤더 + backdrop-blur + Container 본문/푸터), Lobby(Card 그리드 + 확장→아이콘 맵 + canvas float 시그니처 모션 유지), 그리고 Profile/Blog/Projects/Links/Search 전 페이지 재스킨. 모든 페이지별 CSS(5개)는 Tailwind로 흡수·삭제.

### 접근성

§3.7 기준은 그대로 유효 — v1에서 AA 측정을 마친 OKLCH 토큰 값을 그대로 쓰므로 본문 대비 4.5:1 이상 유지. Radix 프리미티브가 포커스 트랩·aria·키보드를 담당. `prefers-reduced-motion`은 canvas float 폴백뿐 아니라 모든 Card hover lift에도 `motion-reduce:` 변형으로 적용.
