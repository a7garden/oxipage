import { useEffect, useState } from "react";
import { Monitor, Moon, Sun } from "lucide-react";

import {
  type ConsoleAppearance,
  type ResolvedMode,
  applyThemeMode,
  getConsoleAppearance,
  getResolvedConsoleMode,
  setConsoleAppearance,
  watchSystemAppearance,
} from "./theme";
import { Button } from "./ui/button";

/** Cycle order: system → light → dark → system. */
const ORDER: ConsoleAppearance[] = ["system", "light", "dark"];

const ICONS: Record<ConsoleAppearance, typeof Monitor> = {
  system: Monitor,
  light: Sun,
  dark: Moon,
};

const LABELS: Record<ConsoleAppearance, string> = {
  system: "System",
  light: "Light",
  dark: "Dark",
};

export function ThemeToggle() {
  const [appearance, setAppearanceState] = useState<ConsoleAppearance>(getConsoleAppearance);
  const [resolved, setResolved] = useState<ResolvedMode>(getResolvedConsoleMode);

  useEffect(() => {
    return watchSystemAppearance(setResolved);
  }, []);

  // Re-resolve when appearance changes.
  useEffect(() => {
    setResolved(getResolvedConsoleMode());
  }, [appearance]);

  // Next mode in the system → light → dark → system cycle.
  const after = ORDER[(ORDER.indexOf(appearance) + 1) % ORDER.length];
  const Icon = ICONS[appearance];

  return (
    <Button
      type="button"
      variant="ghost"
      size="icon"
      onClick={() => {
        setConsoleAppearance(after);
        setAppearanceState(after);
        applyThemeMode(after === "system" ? getResolvedConsoleMode() : after);
      }}
      className="size-8 rounded-md text-muted hover:text-foreground hover:bg-surface"
      title={
        appearance === "system"
          ? `Theme: System (now ${resolved}) — click for ${LABELS[after]}`
          : `Theme: ${LABELS[appearance]} — click for ${LABELS[after]}`
      }
      aria-label={`Theme: ${LABELS[appearance]}. Click to switch to ${LABELS[after]}.`}
    >
      <Icon className="size-4" />
    </Button>
  );
}
