# 접근성 (Accessibility) — WCAG 2.1 AA 측정 결과

> 2026-07-27 갱신. 본 문서는 `web/src/shared/tokens.css` 의 OKLCH 토큰을
> sRGB로 변환해 WCAG 2.1 AA 대비율(일반 텍스트 ≥ 4.5:1, 큰 텍스트 ≥ 3:1)을
> 측정한 결과를 기록한다.

## 1. 측정 방법

- 토큰 OKLCH → Oklab → linear sRGB → gamma sRGB (Björn Ottosson 변환).
- WCAG 상대 휘도 공식으로 대비율 계산.
- 코드는 본 세션 `eval` 셀에 구현 (동일 공식이 `apca` 라이브러리와 일치 확인).
- 측정 시점: doc/08 §8.7+§8.2 작업 이후 커밋(4d2bd47) 기준.

## 2. 토큰 정의 (tokens.css)

```
:root
  --p-neutral-0/50/100/300/500/700/900/950
  --p-accent-400/500/600/700
  --p-gold-500: oklch(78% 0.15 85)
  --p-gold-600: oklch(55% 0.12 65)  ← 2026-07-27 조정 (구값 68% 0.15 85)
  --p-danger-500 / --p-success-500

[data-theme="dark"]
  --color-text-tertiary: oklch(65% 0.012 265)  ← 2026-07-27 조정 (구값 --p-neutral-500)
```

## 3. 측정 결과 (라이트 모드)

| 페어 | 전경 | 배경 | 비 | 비율 | 판정 |
|---|---|---|---|---|---|
| text-primary / canvas | `#0e1218` | `#fbfaf7` | – | **17.99:1** | PASS |
| text-primary / surface-50 | `#0e1218` | `#f0eeea` | – | **16.20:1** | PASS |
| text-primary / surface-raised | `#0e1218` | `#ffffff` | – | **18.78:1** | PASS |
| text-secondary / canvas | `#373b41` | `#fbfaf7` | – | **10.79:1** | PASS |
| text-secondary / surface-50 | `#373b41` | `#f0eeea` | – | **9.72:1** | PASS |
| text-tertiary / canvas | `#74726a` | `#fbfaf7` | – | **4.62:1** | PASS |
| text-tertiary / surface-raised | `#74726a` | `#ffffff` | – | **4.82:1** | PASS |
| white / accent-600 (button) | `#ffffff` | `#6d48d1` | – | **6.00:1** | PASS |
| gold-600 / canvas (별점) | `#a0600b` | `#fbfaf7` | – | **4.82:1** | PASS |
| gold-600 / surface-raised | `#a0600b` | `#ffffff` | – | **5.03:1** | PASS |
| gold-600 / surface-50 | `#a0600b` | `#f0eeea` | – | 4.34:1 | ⚠ 미세 미달 — 별점이 card 배경 위에 놓이지 않음 (전제) |

## 4. 측정 결과 (다크 모드)

| 페어 | 전경 | 배경 | 비 | 비율 | 판정 |
|---|---|---|---|---|---|
| text-primary / canvas | `#fbfaf7` | `#04070f` | – | **19.30:1** | PASS |
| text-primary / surface | `#fbfaf7` | `#0e1218` | – | **17.99:1** | PASS |
| text-primary / surface-raised | `#fbfaf7` | `#171b22` | – | **16.54:1** | PASS |
| text-secondary / canvas | `#b0aea7` | `#04070f` | – | **9.07:1** | PASS |
| text-secondary / surface | `#b0aea7` | `#0e1218` | – | **8.46:1** | PASS |
| text-tertiary / canvas | `#8c8f97` | `#04070f` | – | **6.23:1** | PASS (조정 후) |
| text-tertiary / surface | `#8c8f97` | `#0e1218` | – | **5.80:1** | PASS (조정 후) |
| text-tertiary / surface-raised | `#8c8f97` | `#171b22` | – | **5.34:1** | PASS (조정 후) |
| bg / accent-400 | `#04070f` | `#b7a7ff` | – | **9.58:1** | PASS |
| bg / accent-500 | `#04070f` | `#886bee` | – | **5.17:1** | PASS |
| text-primary / gold-500 | `#fbfaf7` | `#e3ae28` | – | **1.94:1** | 별점 fill vs 텍스트: 의미 없음 |
| gold-500 / canvas (별점 fill) | `#e3ae28` | `#04070f` | – | **9.93:1** | PASS |
| gold-500 / surface (별점 fill) | `#e3ae28` | `#0e1218` | – | **9.26:1** | PASS |
| gold-500 / surface-raised | `#e3ae28` | `#171b22` | – | **8.51:1** | PASS |

## 5. 2026-07-27 조정 내역

### 5.1 `--p-gold-600`: 68% 0.15 85 → 55% 0.12 65

**이유:** 라이트 모드 별점 fill(`RatingStars`, `--color-rating-fill`)이
canvas/흰 표면 위에서 2.79~2.91:1로 AA 기준(큰 아이콘 ≥ 3.0) 미달이었음.

- 새 값: `oklch(55% 0.12 65)` (rgb `#a0600b`, 더 깊은 갈색-골드 톤)
- canvas vs: 4.82:1 / surface-raised vs: 5.03:1 → AA 일반 텍스트 통과
- **알려진 한계:** surface-50(card 배경) vs 4.34:1 — 미세 미달. 별점 컴포넌트는
  통상 상세 페이지(흰 surface-raised 위) 또는 메인 canvas 위에 표시되므로
  card 위에서는 의도적으로 비사용 가정.
- OKLCH H=85도를 유지하면 L=60% 이하에서 gamut 밖(b<0)으로 튐. H=65도로
  옮겨 gamut 안전 확보.

### 5.2 다크 `--color-text-tertiary`: `--p-neutral-500` → `oklch(65% 0.012 265)`

**이유:** 다크 모드 `--p-neutral-500`(L 55%)은 canvas에서 4.18:1, surface-raised에서
3.58:1로 AA 미달이었음.

- 새 값: `oklch(65% 0.012 265)` (rgb `#8c8f97`)
- canvas vs 6.23:1 / surface vs 5.80:1 / surface-raised vs 5.34:1 → AA 통과
- `--p-neutral-500` 토큰 자체는 라이트 text-tertiary(55% L → canvas 4.62:1 PASS)로
  쓰이므로 전역 변경 금지. 다크 테마 블록에서만 override.

## 6. AA 미달/한계 요약

| 항목 | 비율 | 비고 |
|---|---|---|
| 라이트 gold-600 / surface-50 | 4.34:1 | 별점이 card 위에 놓이지 않는다는 설계 전제. 추후 card 위 별점 도입 시 재검토. |
| 그 외 모든 주요 텍스트/아이콘 쌍 | ≥ 4.5:1 | AA 일반 텍스트 기준 통과 |

## 7. 차후 검증 필요 항목

- 브라우저 라이트/다크 전환 FOUC (Phase 0 완료, 스모크 필요 — doc/08 §8.6).
- `prefers-reduced-motion`: 로비 canvas → grid 폴백 (Lobby.tsx).
- VoiceOver/NVDA 주요 화면 읽기 (RatingStars, ThemeToggle, SearchInput).
- 키보드 Tab 순서 / 포커스 가시성.

## 8. 재측정 방법

```python
# OKLCH → sRGB → WCAG ratio. 본 세션 eval 셀에 구현됨.
# 핵심 변환:
#   oklab(L, a, b) → linear sRGB → gamma sRGB → relative luminance → ratio
# Ottosson 역변환 행렬 + WCAG 2.1 공식 사용.
```
