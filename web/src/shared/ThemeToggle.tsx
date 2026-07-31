import { useEffect, useState } from "react";
import { Monitor, Moon, Sun } from "lucide-react";

import {
  type ConsoleAppearance,
  type ResolvedMode,
  getConsoleAppearance,
  getResolvedConsoleMode,
  setConsoleAppearance,
  watchSystemAppearance,
} from "./theme";
import { Button } from "./ui/button";

const options: { value: ConsoleAppearance; label: string; icon: typeof Monitor }[] = [
  { value: "system", label: "System", icon: Monitor },
  { value: "light", label: "Light", icon: Sun },
  { value: "dark", label: "Dark", icon: Moon },
];

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

  function pick(next: ConsoleAppearance) {
    setConsoleAppearance(next);
    setAppearanceState(next);
  }

  return (
    <div
      role="radiogroup"
      aria-label="Console appearance"
      className="inline-flex items-center rounded-md border border-line p-0.5 gap-0.5"
    >
      {options.map(({ value, label, icon: Icon }) => {
        const active = appearance === value;
        return (
          <Button
            key={value}
            type="button"
            role="radio"
            aria-checked={active}
            onClick={() => pick(value)}
            className={`h-7 px-2 text-xs ${active ? "bg-primary text-primary-foreground" : "hover:bg-surface"}`}
            title={
              value === "system"
                ? `System (currently ${resolved})`
                : label
            }
          >
            <Icon className="size-3.5" />
            <span className="ml-1.5 hidden sm:inline">{label}</span>
          </Button>
        );
      })}
    </div>
  );
}
