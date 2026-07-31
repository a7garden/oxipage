# Admin Theme System — Design Spec

> **Date:** 2026-07-31
> **Subproject:** 2 of 5
> **Predecessor:** Console runtime and routing foundation
> **Primary surfaces:** AdminApp, Topbar, Settings, ThemesPage, shared tokens, site theme APIs

## 1. Goal

Complete the Admin theme system without conflating two different preferences: the console's own light/dark appearance and each site's public visual theme. The Admin app must initialize through the shared theme controller, Settings must expose appearance controls, Topbar must use the same controller, and every sidebar color must resolve through light/dark theme tokens.

## 2. Current state

- `admin.html` performs early FOUC prevention using `oxipage-theme` and system preference.
- Public `App.tsx` calls `applyServerTheme()`; `AdminApp` does not.
- `Topbar.tsx` defines a private toggle that writes `data-theme` and localStorage directly.
- Shared `ThemeToggle` already uses `shared/theme.ts` and follows system changes when no explicit stored value exists.
- `SettingsPage` has no theme controls.
- Sidebar variables are defined once in `:root`, intentionally forcing a dark sidebar, while remaining hard-coded color literals still exist in Sidebar, fallback UI, and status controls.
- Theme catalogs disagree across core HTTP, setup, per-site validation, and Admin ThemesPage.
- `--accent-hue` is set by JS but is not consumed by the accent tokens.
- Public global theme GET reads a different DB from the per-site theme endpoint.

## 3. State ownership and precedence

### 3.1 Console appearance

```ts
type ConsoleAppearance = "system" | "light" | "dark";
```

Persistence:

```text
localStorage["oxipage-console-appearance"]
```

Resolution:

```text
explicit light/dark → that mode
system              → prefers-color-scheme
missing/invalid     → system
```

Only console appearance controls `<html data-theme>` in the Admin SPA.

### 3.2 Public site theme

```text
theme_id in the selected site's SQLite theme_config singleton
```

It controls public presentation palette/accent, default public mode metadata, Admin draft preview palette, and static build theme metadata. It does not overwrite an explicit Admin console appearance.

### 3.3 `applyServerTheme()` semantics

`AdminApp` calls `applyServerTheme()` on page load as required. It fetches default/selected site theme metadata and publishes the definition to preview consumers. It never writes the console root's appearance mode or global accent variables.

When the selected slug changes, it loads that site's theme. At `/sites` it loads the default site if one exists, otherwise `paper` metadata. `DraftPreviewPane` applies that definition on an isolated wrapper (`data-public-theme` plus scoped CSS custom properties), so selecting a public theme cannot recolor the Admin shell.

## 4. Early FOUC boot

Both HTML entries load one shared early boot helper before CSS. For Admin, it reads `oxipage-console-appearance`. For the public static SPA, generated build metadata supplies the public default theme/mode. The helper is dependency-free and executes before React.

The duplicated inline scripts are removed. The boot helper and runtime controller share keys and parsing rules.

## 5. Single theme catalog

Create `oxipage-core::theme`:

```rust
pub struct ThemeDefinition {
    pub id: &'static str,
    pub name_ko: &'static str,
    pub name_en: &'static str,
    pub mode: ThemeMode,
    pub accent_hue: f64,
    pub preview_colors: [&'static str; 4],
}
```

Supported IDs are the union of already exposed values:

```text
paper, midnight, sepia, forest, neon, canvas
```

Every definition receives complete mode, hue, label, description, and preview colors. Setup, `/themes`, theme GET/PUT validation, and ThemesPage consume it. Local Rust validators/catalogs and the TS `ThemesPage` constant are removed.

## 6. Theme APIs

```text
GET /api/console/themes
```

Returns all shared definitions.

```text
GET /api/console/theme
```

Move this route from the core-only router into the console router, where `SiteRegistry` is available. It resolves the registered default site and reads that site's DB. With no registered site it returns the `paper` definition without creating a second global theme row. The old core handler that reads `AppState.db` is removed; there is one default-theme endpoint, not two merged handlers at the same path.

```text
GET /api/console/s/{slug}/theme
PUT /api/console/s/{slug}/theme {"theme_id":"forest"}
```

GET returns the full definition plus `theme_id`. PUT uses the shared catalog and returns 400 for unknown IDs.

## 7. Shared browser controller

`web/src/shared/theme.ts` exports:

```ts
getConsoleAppearance(): ConsoleAppearance
setConsoleAppearance(value: ConsoleAppearance): void
getResolvedConsoleMode(): "light" | "dark"
watchSystemAppearance(callback): unsubscribe
applyThemeMode(mode): void
applyServerTheme(slug?: string): Promise<ThemeDefinition>
getThemePalette(theme): Record<string, string>
```

`ThemeToggle` becomes a three-state menu/control. `Topbar` imports it; the private copy is deleted.

`--accent-hue` drives actual OKLCH accent primitives inside public-theme scopes. `DraftPreviewPane` supplies it on its wrapper; generated public HTML supplies it at its root. The Admin shell keeps its own console semantic accent and mode.

## 8. Settings and ThemesPage UX

### Settings > Appearance

```text
Console appearance
[ System ] [ Light ] [ Dark ]

Public site theme
Paper
[Open full theme editor]
```

Appearance applies immediately and persists locally. Public theme summary uses the same per-site query as ThemesPage. The link opens `/s/{slug}/themes`. Settings does not store another `theme_id` in TOML.

### ThemesPage

- Fetches the server catalog.
- Shows all six themes.
- Uses the site's current theme endpoint.
- Applies selection only through PUT.
- Uses an isolated public presentation theme scope for preview; it never mutates global Admin tokens.
- Updates shared query state after mutation.

## 9. Theme-aware sidebar tokens

Move sidebar tokens into `[data-theme="light"]` and `[data-theme="dark"]` scopes:

```text
--console-sidebar-bg
--console-sidebar-text
--console-sidebar-text-hover
--console-sidebar-text-active
--console-sidebar-border
--console-sidebar-border-active
--console-sidebar-hover-bg
--console-sidebar-active-bg
--console-sidebar-label
```

Light mode gets a light neutral sidebar with readable dark text. Dark mode retains the current dark direction. Both meet WCAG AA for navigation text and active state.

Replace literals in Sidebar, ShellFallback, Topbar wordmark, sidebar footer/borders, and theme/status states touched by this subproject. Status colors use semantic success/danger tokens.

## 10. Static build integration

The build manifest stores `theme_id`. Generated static HTML receives theme metadata before first paint. Static Pages do not call a missing runtime theme endpoint.

A build after theme change is required to update deployment. Deploy preflight reports an incompatible build when manifest `theme_id` differs from the current site DB.

## 11. File map

```text
crates/oxipage-core/src/
├── theme.rs
├── http.rs                          # catalog; remove global DB theme handler
└── setup.rs
crates/oxipage-console/src/
├── router.rs                        # default-site GET /theme
└── per_site.rs                      # selected-site GET/PUT
web/
├── theme-boot.js
├── admin.html
├── index.html
└── src/
    ├── shared/theme.ts
    ├── shared/ThemeToggle.tsx
    ├── shared/tokens.css
    └── admin/
        ├── App.tsx
        ├── shell/Topbar.tsx
        ├── shell/Sidebar.tsx
        ├── settings/SettingsPage.tsx
        └── themes/ThemesPage.tsx
```

## 12. Verification

- Hard-load Admin with each stored appearance and observe no opposite-mode flash.
- `system` follows OS preference changes; explicit modes do not.
- Topbar and Settings show the same appearance.
- Admin boot calls the server theme loader; site changes load the selected site's definition.
- Every catalog ID appears once, is accepted by PUT, and renders in ThemesPage.
- Unknown IDs return 400.
- Changing hue visibly affects scoped presentation; `--accent-hue` is not dead.
- Light/dark sidebar active, hover, label, footer, and focus states meet contrast requirements.
- A static build uses the stored public theme without runtime theme API failures.
