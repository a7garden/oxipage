# 디자인 시스템 v2 — Radix + Tailwind v4 + OKLCH 토큰

**날짜:** 2026-07-27
**상태:** 승인됨 (B1 방식, 사용자 자리 비움 중 자율 진행)
**선행 문서:** `doc/03-design-system.md` (v1 컨셉), `docs/accessibility.md` (AA 측정)

## 1. 배경과 목표

### 진단
v1 디자인 시스템은 기술적 토대가 견고(OKLCH 3계층 토큰, WCAG AA 측정 완료, FOUC 방지, `data-theme` 라이트/다크)하지만, 비주얼 실현도가 컨셉에 못 미침. 사용자가 식별한 5개 갭:
1. 컴포넌트가 밋밙하다 (elevation·hover·전환 없음)
2. 타이포그래피 위계가 약하다 (display 폰트 부재, 단조 스케일)
3. 레이아웃이 단조롭다 (여백·그리드 변화 부족)
4. 비주얼 포인트 부족 (이미지·썸네일 회색)
5. 컨셉을 더 풍부하게 (질감·마이크로인터랙션)

`global.css`가 94줄, 재사용 컴포넌트 클래스 손에 꼽힘 — 컴포넌트 레이어가 빈약.

### 목표
**컨셉(종이/잉크 + 딥 인디고-바이올렛)은 보존**하면서, 헤드리스 컴포넌트 라이브러리(Radix) + Tailwind v4 유틸리티를 올려 **컴포넌트 레이어의 풍요도와 일관성을 끌어올린다.**

## 2. 스택 결정 (섹션 1)

- **헤드리스 프리미티브:** Radix UI (a11y 검증, unstyled, shadcn 기반). React 19.1 + Bun 1.3에서 friction 없이 설치 확인(`bun add @radix-ui/react-dialog` → 1.7초, peer-dep 경고 없음).
- **스킨 방식:** Tailwind v4 (`@theme inline`) + class-variance-authority. **shadcn CLI를 쓰지 않고 패턴만 차용** — CLI는 자체 토큰을 덮어쓰므로 우리 OKLCH 시스템과 충돌. 대신 컴포넌트는 shadcn v4 패턴(`React.ComponentProps<>` + `data-slot`, forwardRef 제거)을 따라 손수 작성.
- **핵심 기술적 결정:** `@theme inline`(plain `@theme` 아님). plain은 빌드타임에 값을 베이크하여 `data-theme` 런타임 전환을 깨뜨림. `inline`은 `var()` 참조를 유지하여 토큰 스위칭이 자동 추종.
- **추가 의존성:** `class-variance-authority`, `clsx`, `tailwind-merge`, `tw-animate-css`, `lucide-react`, 필요한 Radix 패키지(`@radix-ui/react-slot|avatar|separator|tooltip|tabs|label|dropdown-menu`).

## 3. 토큰 매핑 전략 (섹션 2)

### 보존
기존 OKLCH `--p-*`(primitive)와 `--color-*`(semantic)를 100% 보존. 이미 AA 측정이 끝난 자산이므로 값 변경 최소화.

### Tailwind 노출
`@theme inline` 블록에서 시맨틱 토큰을 Tailwind 색상으로 별칭:
```css
@theme inline {
  --color-canvas:        var(--color-bg-canvas);
  --color-surface:       var(--color-bg-surface);
  --color-surface-raised:var(--color-bg-surface-raised);
  --color-text-primary:  var(--color-text-primary);
  --color-text-secondary:var(--color-text-secondary);
  --color-text-tertiary: var(--color-text-tertiary);
  --color-border:        var(--color-border);
  --color-accent:        var(--color-accent);
  --color-accent-contrast:var(--color-accent-contrast);
  --color-rating-fill:   var(--color-rating-fill);
  --color-danger:        var(--color-danger);
  --color-success:       var(--color-success);
  /* 폰트·반경·그림자도 동일 패턴 */
}
```
→ `bg-canvas`, `text-primary`, `border-border`, `bg-accent` 유틸리티가 모두 `data-theme` 전환을 자동 추종.

### 추가 토큰 (5개 갭 해결)

**Elevation** (갭 1, 5) — 라이트는 검은 잉크 그림자, 다크는 더 깊은 검은 + 미세 border로 보정:
```css
--p-neutral-999: oklch(0% 0 0); /* 그림자용 pure black */
[data-theme="light"] {
  --shadow-xs: 0 1px 2px oklch(0% 0 0 / 0.04);
  --shadow-sm: 0 1px 3px oklch(0% 0 0 / 0.07), 0 1px 2px oklch(0% 0 0 / 0.04);
  --shadow-md: 0 4px 8px oklch(0% 0 0 / 0.08), 0 2px 4px oklch(0% 0 0 / 0.04);
  --shadow-lg: 0 12px 24px oklch(0% 0 0 / 0.10), 0 4px 8px oklch(0% 0 0 / 0.06);
}
[data-theme="dark"] {
  --shadow-xs: 0 1px 2px oklch(0% 0 0 / 0.30);
  --shadow-sm: 0 1px 3px oklch(0% 0 0 / 0.40), 0 1px 2px oklch(0% 0 0 / 0.30);
  --shadow-md: 0 4px 8px oklch(0% 0 0 / 0.45), 0 2px 4px oklch(0% 0 0 / 0.35);
  --shadow-lg: 0 12px 24px oklch(0% 0 0 / 0.50), 0 4px 8px oklch(0% 0 0 / 0.40);
}
```

**Spacing scale** (갭 3) — 현재 하드코딩된 rem을 체계화:
`--space-1..12` (0.25rem → 3rem, 4px 기반).

**Radius scale** (갭 1) — `--radius-xs/sm/md/lg/xl` (0.25rem → 1.25rem), 기존 `--radius-md:0.75rem` 흡수.

**Type scale** (갭 2) — `--text-xs/sm/base/lg/xl/2xl/3xl/display` + 매칭 line-height.

**Display 폰트** (갭 2) — Fraunces(라틴 디스플레이 세리프) + Noto Serif KR(한글 세리프) 쌍. 본문은 Pretendard 유지. `--font-display` 토큰 추가.

**Motion** (갭 5) — `--duration-fast:120ms`, `--duration-base:200ms`, `--ease-out:cubic-bezier(0.16,1,0.3,1)`.

## 4. 컴포넌트 카탈로그 (섹션 3)

`web/src/shared/ui/` 아래. 모두 shadcn v4 패턴(`React.ComponentProps<>`, `data-slot`, forwardRef 없음).

| 컴포넌트 | 용도 | 비고 |
|---|---|---|
| `cn` (util) | clsx + tailwind-merge | 모든 컴포넌트의 className 병합 |
| `Button` | cva variants: primary/secondary/outline/ghost/link, size sm/md/lg/icon | Radix Slot으로 asChild 지원 |
| `Card` | Header/Title/Description/Content/Footer | elevation 토큰 사용, hover 옵션 |
| `Input` / `Textarea` | 폼 컨트롤 | focus ring = accent |
| `Label` | 폼 라벨 | Radix Label |
| `Badge` | variant: default/secondary/outline/accent | 상태·태그 표시 |
| `Avatar` | 이미지+폴백 | Radix Avatar |
| `Separator` | 구분선 | Radix Separator |
| `Skeleton` | 로딩 placeholder | pulse 애니메이션 |
| `Tooltip` | 툴팁 | Radix Tooltip |
| `Tabs` | 탭 | Radix Tabs (로비 모드 전환 등) |
| `Container` | max-width 래퍼 | 64rem 유지 |
| `PageHeader` | title+description+actions | 페이지 상단 패턴 |
| `EmptyState` | 빈 목록 placeholder | 갭 4 비주얼 보강 |
| `RatingStars` (리팩터) | 기존 컴포넌트를 새 시스템으로 | gold 토큰 유지 |
| `ThemeToggle` (리팩터) | 기존, Button 컴포넌트 사용 | FOUC 로직 유지 |

## 5. 타이포그래피 (섹션 4)

| 역할 | 폰트 | 토큰 |
|---|---|---|
| Display (히어로·섹션 타이틀) | **Fraunces** → Noto Serif KR → serif | `--font-display` |
| Body/UI (본문·버튼·네비) | Pretendard Variable (유지) | `--font-body` |
| Mono (코드·커밋피드) | 시스템 mono (유지) | `--font-mono` |

로딩: `index.html`에 Google Fonts `<link rel="stylesheet">` (preconnect + display=swap). Fraunces는 variable, weight 400/500/600/700 + 소프트니스/광학축은 기본.

## 6. 레이아웃 & 비주얼 패턴 (섹션 5)

- **Container:** 64rem 유지, 반응형 패딩 `clamp(1rem, 4vw, 3rem)`.
- **PageHeader:** 페이지마다 일관된 title/description/actions 슬롯.
- **Card elevation:** 기본 `shadow-sm`, hover 시 `shadow-md` + `translate-y-[-2px]` (절제된 lift).
- **List/Grid 변형:** 로비 3모드(list/grid/canvas) 유지, grid 카드에 비주얼 커버 슬롯 추가(갭 4).
- **EmptyState:** 아이콘(lucide) + 문구 + CTA로 빈 목록을 풍부하게.
- **마이크로인터랙션:** `prefers-reduced-motion` 존중 — 모션 토큰은 reduce일 때 0ms로 폴백.

## 7. 마이그레이션 전략 (섹션 6)

단계별 진행, **각 단계 끝에 커밋** (롤백 단위). 사용자 자리 비움 중이므로 "깨진 빌드로 일어나는" 최악 사례 방지.

1. **Foundation + PoC** — Tailwind/Radix/cva 설치, vite/tsconfig alias, `tokens.css` 재작성(`@theme inline`), `cn` util, `Button` 1개로 full-chain 검증(tsc + build + 라이트/다크 OKLCH 렌더링). **PoC 실패 시 B2(vanilla CSS)로 회귀.**
2. **핵심 컴포넌트** — Card/Input/Label/Badge/Avatar/Separator/Skeleton/Tooltip/Tabs + Container/PageHeader/EmptyState.
3. **타이포 + 폰트** — Fraunces/Noto Serif KR 로딩, type scale 적용.
4. **App shell + Lobby** — App.tsx 헤더/셸 재스킨, Lobby 3모드 재스킨.
5. **확장 페이지** — Profile, Blog(목록/포스트), Projects(목록/상세), Links, Search 각 재스킨, 페이지별 CSS를 Tailwind로 흡수.
6. **정리 + 검증** — 잔존 CSS 정리, full build/typecheck, 런타임 스모크, 디자인 문서 갱신.

## 8. 비목표 (Out of Scope)

- 컬러 팔레트 자체 변경 (OKLCH 값은 보존)
- 백엔드/SSR 변경
- 로비 canvas 모드의 물리 시뮬레이션 고도화 (기존 drift 유지)
- 모션 과잉 (절제 원칙 유지)

## 9. 검증 계획

각 단계:
- `bun run build` (tsc --noEmit + vite build) 통과
- `bun run dev`에서 라이트/다크 전환 시 OKLCH 값이 올바르게 스위칭되는지 브라우저 확인
- 기존 기능(라우팅, API 호출, 다국어) 회귀 없음

최종 단계: 디자인 문서(`doc/03-design-system.md`)에 v2 섹션 추가, `docs/accessibility.md` 갱신(새 컴포넌트 대비율 재측정 필요 시).

## 10. 롤백

- 백엔드 WIP(7개 Rust 파일 + oxipage.toml + 신규 migration)는 사용자 소유 — 건드리지 않음.
- 본 작업은 `7def3df` HEAD 위에 커밋이 쌓임, 단계별 revert 가능.
- PoC(Phase 1) 통과 전까지 기존 파일을 덮어쓰지 않음 — 새 파일 우선, 교체는 검증 후.
