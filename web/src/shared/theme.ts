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
