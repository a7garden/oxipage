import { useEffect, useState } from "react";
import { Moon, Sun } from "lucide-react";

import { type Theme, getEffectiveTheme, toggleTheme, watchSystemTheme } from "./theme";
import { Button } from "./ui/button";

export function ThemeToggle() {
  const [theme, setTheme] = useState<Theme>(() => getEffectiveTheme());

  useEffect(() => watchSystemTheme(setTheme), []);

  const isDark = theme === "dark";

  return (
    <Button
      type="button"
      variant="ghost"
      size="icon"
      aria-label={isDark ? "라이트 모드로 전환" : "다크 모드로 전환"}
      onClick={() => setTheme(toggleTheme())}
    >
      {isDark ? <Sun /> : <Moon />}
    </Button>
  );
}
