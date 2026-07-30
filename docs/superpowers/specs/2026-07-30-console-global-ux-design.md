# Console Global UX — Design Spec

> **Date:** 2026-07-30
> **Sub-project:** 5 of the decomposed "remaining console work" (Phases 12 + 14).
> **Scope:** CSS token cleanup, scroll reset, offline indicator, markdown preview, SiteSelector info panel, responsive decision.
> **Predecessor:** S1–S3 (this polish layer ideally lands after pages carry real data).

## 1. Goal

Round out the console shell's cross-cutting UX: replace hardcoded colors with proper design tokens, reset scroll on navigation, surface offline state, add a markdown preview to content editors, and enrich the site selector with live stats.

## 2. Scope

### In scope
- **Console CSS tokens (P12):** define `--console-sidebar-*` variables; replace hardcoded hex in Sidebar/Topbar.
- **Scroll reset (P14):** `pathname`-based scroll-to-top on navigation.
- **Offline indicator (P14):** `navigator.onLine` banner.
- **Markdown preview (P14):** `marked`-powered editor in content body fields.
- **SiteSelector info panel (P12):** URL + last-build + content count via S1's `getStats`.

### Out of scope (deferred)
- **`build_log.finished_at`:** **absorbed by S3** (the deploy pipeline adds it). Not duplicated here.
- **Tablet/mobile responsive (#1):** deferred. `console-shell-redesign.md` §8 states "v1은 desktop only". A full responsive collapse is a separate polish sub-project; if done in S5 at all, it is limited to a sidebar hamburger under `lg:`.

## 3. Current State (grounding)

| Concern | Current state | File |
|---------|--------------|------|
| Shell layout | `Topbar` + `flex(Sidebar + main)`; `<main>` `overflow-auto p-6`; no scroll reset | `web/src/admin/shell/ConsoleShell.tsx` |
| Sidebar colors | **hardcoded hex** inline: `#1a1e24` bg, `#6b7280` label, `#4ade80` active text, `#22c55e` active border, `#9ca3af` inactive | `web/src/admin/shell/Sidebar.tsx:34-66` |
| Topbar logo | hardcoded `#2a6b4a` | `web/src/admin/shell/Topbar.tsx:42` |
| `--console-sidebar-*` tokens | proposed in shell-redesign §5.3, **never implemented** | `web/src/shared/tokens.css` |
| Router | **`BrowserRouter`** (component router) — `ScrollRestoration` (data-router-only) is **not usable** | `web/src/admin/App.tsx:2` |
| SiteSelector | name + path only; no URL/last-build/count | `web/src/admin/shell/SiteSelector.tsx:39-53` |
| `marked` | **not installed** | `web/package.json` |
| Drawer editor | generic `children` slot; body fields are bare `<Textarea>` | `web/src/shared/ui/drawer.tsx`, `content/*Tab.tsx` |

## 4. Design

### 4.1 Console CSS tokens (P12)

Add to `tokens.css` `:root` (theme-agnostic — sidebar stays dark regardless of toggle, per shell-redesign §5.3):
```css
:root {
  --console-sidebar-bg: #1a1e24;
  --console-sidebar-text: #9ca3af;
  --console-sidebar-text-active: #4ade80;
  --console-sidebar-border-active: #22c55e;
  --console-sidebar-hover-bg: rgba(255, 255, 255, 0.04);
  --console-sidebar-label: #6b7280;
}
```
- `Sidebar.tsx`: replace every inline hex with `var(--console-sidebar-*)`; `backgroundColor` → `style={{ backgroundColor: "var(--console-sidebar-bg)" }}`.
- `Topbar.tsx`: logo `#2a6b4a` → reuse the accent token (e.g. `--p-accent-600` via a `--console-brand` alias) — keep a single source of truth for the pine green.

### 4.2 Scroll reset (P14)

Because the app uses `BrowserRouter` (not a data router), `ScrollRestoration` is unavailable. Add a `ScrollToTop` component:
```tsx
function ScrollToTop() {
  const { pathname } = useLocation();
  useEffect(() => { window.scrollTo(0, 0); }, [pathname]);
  return null;
}
```
- Mount inside the routed tree (in `ConsoleShell` or just inside `<Routes>` in `App.tsx`). Resets the `<main>` scroll on every site/section change.

### 4.3 Offline indicator (P14)

- New `web/src/admin/shared/ui/OfflineBanner.tsx`:
  - State from `navigator.onLine`; subscribe to `online`/`offline` events.
  - Renders a full-width banner ("Offline — changes will retry when reconnected") under the Topbar when offline; nothing when online.
- TanStack Query `networkMode` defaults to `onlineFirst` (mutations already retry on reconnect) — no global config change required; document this.
- Mount in `ConsoleShell` between `<Topbar />` and the flex row.

### 4.4 Markdown preview (P14)

- Add `marked` to `web/package.json`.
- New `web/src/admin/shared/ui/MarkdownEditor.tsx` — a Textarea with a `[Edit | Preview]` toggle; Preview renders `marked.parse(value)` into a styled `prose` container.
- Replace the bare body `<Textarea>` in content editors with `<MarkdownEditor>`: blog body, novel chapter body (S2), scraps body, movie reviews. (Integration point per tab confirmed in the plan; the S2 chapter editor adopts this too.)

### 4.5 SiteSelector info panel (P12)

- Extend the dropdown's right-side area (currently absent) with current-site info: site URL, last-build status, total content count.
- Reuse S1's `getStats(slug)` (already returns `counts` + `last_build`) — no new endpoint. Fetch for the current slug; lazy.

### 4.6 Responsive (deferred)

- No work in S5 by default. If scope allows: add a sidebar hamburger under `lg:` (state in `ConsoleShell`, slide-in drawer). Otherwise a follow-up. Documented as deferred to honor the desktop-first stance.

## 5. Constraints

- New tokens go in `:root` (theme-agnostic) so the sidebar never lightens with the theme toggle.
- No new server endpoints — S5 is frontend-only (it consumes S1's `/stats`).
- Reuse existing UI primitives; `MarkdownEditor`/`OfflineBanner`/`ScrollToTop` are thin additions.
- `marked` output is trusted (author's own content in a local console) — no XSS sanitization step required for v1, but note it for any future multi-user path.

## 6. Testing

- **Manual smoke:** toggle dark/light — sidebar stays dark, only main content switches; navigate between pages → scroll resets to top; disable network (DevTools) → banner appears, re-enable → disappears; open a blog post body → Edit/Preview renders markdown; open SiteSelector → current site shows URL + last build + counts.

## 7. File map

```
web/src/
├── shared/tokens.css                     # +--console-sidebar-* tokens (:root)
├── admin/
│   ├── shell/
│   │   ├── ConsoleShell.tsx              # +ScrollToTop, +OfflineBanner mount
│   │   ├── Sidebar.tsx                   # hardcoded hex → var()
│   │   ├── Topbar.tsx                    # logo hex → token
│   │   └── SiteSelector.tsx              # +info panel via getStats
│   ├── shared/ui/
│   │   ├── OfflineBanner.tsx             # NEW
│   │   ├── MarkdownEditor.tsx            # NEW (marked)
│   │   └── ScrollToTop.tsx               # NEW
│   ├── App.tsx                           # mount ScrollToTop in routed tree
│   └── content/*Tab.tsx                  # body Textarea → MarkdownEditor
└── package.json                          # +marked
```
