// Console appearance vs public site theme — kept distinct on purpose.
//
//  - Console appearance (the Admin shell's <html data-theme>)
//      localStorage["oxipage-console-appearance"] = "system" | "light" | "dark"
//      Resolution: explicit light/dark → that mode; "system" or missing/invalid →
//      window.matchMedia('(prefers-color-scheme: dark)').
//
//  - Public site theme (the per-site SQLite singleton)
//      Shared catalog: oxipage_core::theme in Rust; ThemeDefinition here.
//      applyServerTheme() publishes palette variables to the document, but
//      NEVER mutates the console's data-theme or sets console mode.

export type ConsoleAppearance = "system" | "light" | "dark";
export type ResolvedMode = "light" | "dark";

export const STORAGE_KEY = "oxipage-console-appearance";

export interface ThemeDefinition {
  id: string;
  name_ko: string;
  name_en: string;
  mode: ResolvedMode;
  accent_hue: number;
  preview_colors: readonly [string, string, string, string];
  description_ko: string;
  description_en: string;
}

function isAppearance(value: unknown): value is ConsoleAppearance {
  return value === "system" || value === "light" || value === "dark";
}

function systemMode(): ResolvedMode {
  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

export function getConsoleAppearance(): ConsoleAppearance {
  try {
    const v = localStorage.getItem(STORAGE_KEY);
    return isAppearance(v) ? v : "system";
  } catch {
    return "system";
  }
}

export function setConsoleAppearance(value: ConsoleAppearance): void {
  try {
    localStorage.setItem(STORAGE_KEY, value);
  } catch {
    /* storage disabled — ignore */
  }
  applyThemeMode(getResolvedConsoleMode());
}

export function getResolvedConsoleMode(): ResolvedMode {
  const stored = getConsoleAppearance();
  return stored === "light" || stored === "dark" ? stored : systemMode();
}

export function applyThemeMode(mode: ResolvedMode): void {
  document.documentElement.dataset.theme = mode;
}

/** Watch OS appearance. Only fires when stored value is "system" (or missing). */
export function watchSystemAppearance(cb: (mode: ResolvedMode) => void): () => void {
  const mq = window.matchMedia("(prefers-color-scheme: dark)");
  const listener = () => {
    if (getConsoleAppearance() === "system") {
      const m = systemMode();
      applyThemeMode(m);
      cb(m);
    }
  };
  mq.addEventListener("change", listener);
  return () => mq.removeEventListener("change", listener);
}

/**
 * Fetch the default site's theme metadata and publish palette variables to
 * <html>. Never overwrites console's data-theme or console mode.
 *
 * @param slug  Optional slug. When undefined, hits the default-site endpoint
 *              that resolves via SiteRegistry (no slug in URL).
 * @returns     The resolved ThemeDefinition, or null if nothing registered.
 */
export async function applyServerTheme(slug?: string): Promise<ThemeDefinition | null> {
  try {
    const url = slug ? `/api/console/s/${encodeURIComponent(slug)}/theme` : "/api/console/theme";
    const res = await fetch(url);
    if (!res.ok) return null;
    const json = (await res.json()) as { data: { theme_id: string; definition: ThemeDefinition } };
    const def = json?.data?.definition;
    if (!def) return null;
    publishPalette(def);
    return def;
  } catch {
    return null;
  }
}

/** Pure helper: derive the variable map from a ThemeDefinition. */
export function getThemePalette(theme: ThemeDefinition): Record<string, string> {
  return {
    "--accent-hue": String(theme.accent_hue),
    "--public-accent": `oklch(60% 0.14 ${theme.accent_hue})`,
    "--public-surface-bg": theme.preview_colors[0],
    "--public-surface-text": theme.preview_colors[2],
  };
}

function publishPalette(theme: ThemeDefinition): void {
  const root = document.documentElement;
  root.dataset.publicTheme = theme.id;
  // Only set --accent-hue on the document root; OKLCH primitives that
  // depend on it live inside [data-public-theme="..."] scopes.
  root.style.setProperty("--accent-hue", String(theme.accent_hue));
  // Subset of palette for any unscoped consumers
  root.style.setProperty("--public-surface-bg", theme.preview_colors[0]);
  root.style.setProperty("--public-surface-text", theme.preview_colors[2]);
}
