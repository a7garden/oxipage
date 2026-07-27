import { useEffect, useState } from 'react';
import { type Theme, getEffectiveTheme, toggleTheme, watchSystemTheme } from './theme';

export function ThemeToggle() {
  const [theme, setTheme] = useState<Theme>(() => getEffectiveTheme());

  useEffect(() => watchSystemTheme(setTheme), []);

  return (
    <button
      type="button"
      className="theme-toggle"
      aria-label={theme === 'dark' ? '라이트 모드로 전환' : '다크 모드로 전환'}
      onClick={() => setTheme(toggleTheme())}
    >
      {theme === 'dark' ? 'Light' : 'Dark'}
    </button>
  );
}
