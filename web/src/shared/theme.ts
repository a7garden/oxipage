export type Theme = 'light' | 'dark';

const STORAGE_KEY = 'oxipage-theme';

export function getStoredTheme(): Theme | null {
  try {
    const t = localStorage.getItem(STORAGE_KEY);
    return t === 'light' || t === 'dark' ? t : null;
  } catch {
    return null;
  }
}

export function getSystemTheme(): Theme {
  return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
}

export function getEffectiveTheme(): Theme {
  return getStoredTheme() ?? getSystemTheme();
}

export function applyTheme(t: Theme): void {
  document.documentElement.dataset.theme = t;
}

export function setStoredTheme(t: Theme | null): void {
  try {
    if (t === null) localStorage.removeItem(STORAGE_KEY);
    else localStorage.setItem(STORAGE_KEY, t);
  } catch {
    /* 스토리지 불가 환경은 무시 */
  }
}

export function toggleTheme(): Theme {
  const next: Theme = getEffectiveTheme() === 'dark' ? 'light' : 'dark';
  setStoredTheme(next);
  applyTheme(next);
  return next;
}

/** 저장된 선택이 없을 때만 시스템 테마 변경을 추적한다. unsubscribe를 반환. */
export function watchSystemTheme(cb: (t: Theme) => void): () => void {
  const mq = window.matchMedia('(prefers-color-scheme: dark)');
  const listener = () => {
    if (getStoredTheme() === null) {
      const t = getSystemTheme();
      applyTheme(t);
      cb(t);
    }
  };
  mq.addEventListener('change', listener);
  return () => mq.removeEventListener('change', listener);
}

// ─── Server-side theme (doc/12 §12.7) ───

interface ThemeEntry {
  id: string;
  mode: string;
  accent_hue: number;
}

/**
 * 서버에서 현재 사이트 테마를 가져와 CSS 변수를 설정한다.
 * 호출 시점: 애플리케이션 부트 (auth 불필요 — 공개 엔드포인트).
 * light/dark 선택은 localStorage > 시스템 > 서버 테마 순으로 우선순위.
 * 서버 테마는 악센트 색상과 추가 변수를 결정한다.
 */
export async function applyServerTheme(): Promise<void> {
  try {
    const [themeRes, catalogRes] = await Promise.all([
      fetch('/api/v1/theme'),
      fetch('/api/v1/themes'),
    ]);
    if (!themeRes.ok || !catalogRes.ok) return;

    const themeData = await themeRes.json();
    const catalogData = await catalogRes.json();

    const themeId: string = themeData?.data?.theme_id ?? 'paper';
    const catalog: ThemeEntry[] = catalogData?.data ?? [];
    const theme = catalog.find((t: ThemeEntry) => t.id === themeId);
    if (!theme) return;

    // 악센트 색상 (CSS 변수로 주입)
    document.documentElement.style.setProperty('--accent-hue', String(theme.accent_hue));

    // 테마가 제안하는 mode (localStorage 없을 때만 사용)
    const stored = getStoredTheme();
    if (stored === null) {
      // 로컬 저장이 없으면 서버 테마의 mode 사용
      applyTheme(theme.mode as Theme);
    }
  } catch {
    // 네트워크 에러 시 무시 — 로컬 테마 유지
  }
}
