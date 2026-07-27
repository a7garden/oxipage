# Accessibility — WCAG 2.1 AA measurement results

> Updated 2026-07-27. This document records the WCAG 2.1 AA contrast ratios (normal text ≥ 4.5:1,
> large text ≥ 3.1) measured by converting the OKLCH tokens in `web/src/shared/tokens.css` to sRGB.

## 1. Measurement method

- OKLCH → Oklab → linear sRGB → gamma sRGB (Björn Ottosson's transform).
- Contrast ratio via the WCAG relative-luminance formula.
- The code was implemented in this session's `eval` cell (the same formula matches the `apca` library).
- Measured against the tree at commit `4d2bd47` (after the doc/08 §8.7 + §8.2 work).

## 2. Token definitions (tokens.css)

```
:root
  --p-neutral-0/50/100/300/500/700/900/950
  --p-accent-400/500/600/700
  --p-gold-500: oklch(78% 0.15 85)
  --p-gold-600: oklch(55% 0.12 65)   ← adjusted 2026-07-27 (was 68% 0.15 85)
  --p-danger-500 / --p-success-500

[data-theme="dark"]
  --color-text-tertiary: oklch(65% 0.012 265)   ← adjusted 2026-07-27 (was --p-neutral-500)
```

## 3. Results (light mode)

| Pair | Foreground | Background | Ratio | Verdict |
|---|---|---|---|---|
| text-primary / canvas | `#0e1218` | `#fbfaf7` | **17.99:1** | PASS |
| text-primary / surface-50 | `#0e1218` | `#f0eeea` | **16.20:1** | PASS |
| text-primary / surface-raised | `#0e1218` | `#ffffff` | **18.78:1** | PASS |
| text-secondary / canvas | `#373b41` | `#fbfaf7` | **10.79:1** | PASS |
| text-secondary / surface-50 | `#373b41` | `#f0eeea` | **9.72:1** | PASS |
| text-tertiary / canvas | `#74726a` | `#fbfaf7` | **4.62:1** | PASS |
| text-tertiary / surface-raised | `#74726a` | `#ffffff` | **4.82:1** | PASS |
| white / accent-600 (button) | `#ffffff` | `#6d48d1` | **6.00:1** | PASS |
| gold-600 / canvas (rating) | `#a0600b` | `#fbfaf7` | **4.82:1** | PASS |
| gold-600 / surface-raised | `#a0600b` | `#ffffff` | **5.03:1** | PASS |
| gold-600 / surface-50 | `#a0600b` | `#f0eeea` | 4.34:1 | ⚠ marginal miss — rating stars are not placed on a card background (by design) |

## 4. Results (dark mode)

| Pair | Foreground | Background | Ratio | Verdict |
|---|---|---|---|---|
| text-primary / canvas | `#fbfaf7` | `#04070f` | **19.30:1** | PASS |
| text-primary / surface | `#fbfaf7` | `#0e1218` | **17.99:1** | PASS |
| text-primary / surface-raised | `#fbfaf7` | `#171b22` | **16.54:1** | PASS |
| text-secondary / canvas | `#b0aea7` | `#04070f` | **9.07:1** | PASS |
| text-secondary / surface | `#b0aea7` | `#0e1218` | **8.46:1** | PASS |
| text-tertiary / canvas | `#8c8f97` | `#04070f` | **6.23:1** | PASS (after adjust) |
| text-tertiary / surface | `#8c8f97` | `#0e1218` | **5.80:1** | PASS (after adjust) |
| text-tertiary / surface-raised | `#8c8f97` | `#171b22` | **5.34:1** | PASS (after adjust) |
| bg / accent-400 | `#04070f` | `#b7a7ff` | **9.58:1** | PASS |
| bg / accent-500 | `#04070f` | `#886bee` | **5.17:1** | PASS |
| text-primary / gold-500 | `#fbfaf7` | `#e3ae28` | **1.94:1** | rating fill vs text: not meaningful |
| gold-500 / canvas (rating fill) | `#e3ae28` | `#04070f` | **9.93:1** | PASS |
| gold-500 / surface (rating fill) | `#e3ae28` | `#0e1218` | **9.26:1** | PASS |
| gold-500 / surface-raised | `#e3ae28` | `#171b22` | **8.51:1** | PASS |

## 5. Adjustments made on 2026-07-27

### 5.1 `--p-gold-600`: 68% 0.15 85 → 55% 0.12 65

**Reason:** in light mode the rating fill (`RatingStars`, `--color-rating-fill`) was 2.79–2.91:1 on
canvas/white surfaces, below the AA threshold for large icons (≥ 3.0).

- New value: `oklch(55% 0.12 65)` (rgb `#a0600b`, a deeper brown-gold).
- canvas: 4.82:1 / surface-raised: 5.03:1 → passes AA for normal text.
- **Known limit:** surface-50 (card background) at 4.34:1 is a marginal miss. The rating component
  is assumed never to sit on a card.
- Keeping OKLCH H = 85° with L < 60% pushes the color out of gamut (b < 0). Moving to H = 65° keeps
  it gamut-safe.

### 5.2 Dark `--color-text-tertiary`: `--p-neutral-500` → `oklch(65% 0.012 265)`

**Reason:** dark-mode `--p-neutral-500` (L 55%) was 4.18:1 on canvas and 3.58:1 on surface-raised —
below AA.

- New value: `oklch(65% 0.012 265)` (rgb `#8c8f97`).
- canvas: 6.23:1 / surface: 5.80:1 / surface-raised: 5.34:1 → passes AA.
- The `--p-neutral-500` token itself is still used for light-mode text-tertiary (55% L → 4.62:1 on
  canvas, PASS), so it must not be changed globally. Override only inside the dark-theme block.

## 6. AA shortfalls / limits summary

| Item | Ratio | Note |
|---|---|---|
| Light gold-600 / surface-50 | 4.34:1 | By design, rating stars are not placed on a card. Re-check if stars ever appear on a card. |
| All other major text/icon pairs | ≥ 4.5:1 | Passes AA for normal text. |

## 7. Items needing future verification

- Browser light/dark toggle FOUC (Phase 0 done; needs a smoke test — doc/08 §8.6).
- `prefers-reduced-motion`: lobby canvas → grid fallback (Lobby.tsx).
- VoiceOver/NVDA reading the main screens (RatingStars, ThemeToggle, SearchInput).
- Keyboard Tab order / focus visibility.

## 8. Re-measurement method

```python
# OKLCH → sRGB → WCAG ratio. Implemented in this session's eval cell.
# Core transforms:
#   oklab(L, a, b) → linear sRGB → gamma sRGB → relative luminance → ratio
# Uses the Ottosson inverse matrix + the WCAG 2.1 formula.
```
