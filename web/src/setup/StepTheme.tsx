// Step 5: 테마 & 레이아웃 (doc/13 §13.7.2)

import { useState } from "react";
import { Button } from "../shared/ui/button";
import type { ThemeInfo } from "./api";

interface Props {
  themes: ThemeInfo[];
  onNext: (data: { theme_id: string; lobby_mode?: string }) => void;
  onBack: () => void;
  loading: boolean;
}

const LAYOUTS = [
  { id: "grid", icon: "⊞", label: "Grid" },
  { id: "list", icon: "☰", label: "List" },
  { id: "canvas", icon: "✦", label: "Canvas" },
];

export function StepTheme({ themes, onNext, onBack, loading }: Props) {
  const [themeId, setThemeId] = useState(
    window.matchMedia("(prefers-color-scheme: dark)").matches ? "midnight" : "paper",
  );
  const [layout, setLayout] = useState("grid");

  return (
    <div>
      <h2 className="text-xl font-semibold mb-6 text-center">테마 & 레이아웃</h2>

      <label className="block text-sm font-medium mb-3">테마</label>
      <div className="grid grid-cols-2 gap-3 mb-8">
        {themes.map((t) => (
          <button
            key={t.id}
            onClick={() => setThemeId(t.id)}
            className={`p-3 rounded-lg border text-left transition-all ${
              themeId === t.id
                ? "border-primary ring-2 ring-primary/20"
                : "border-line hover:bg-surface"
            }`}
          >
            {/* Color preview bar */}
            <div className="flex gap-1 mb-2">
              {t.preview_colors.map((c, i) => (
                <div key={i} className="h-6 flex-1 rounded" style={{ backgroundColor: c }} />
              ))}
            </div>
            <div className="text-sm font-medium">{t.name_ko}</div>
            <div className="text-xs text-subtle">{t.name_en}</div>
          </button>
        ))}
      </div>

      <label className="block text-sm font-medium mb-3">로비 레이아웃</label>
      <div className="grid grid-cols-3 gap-3 mb-8">
        {LAYOUTS.map((l) => (
          <button
            key={l.id}
            onClick={() => setLayout(l.id)}
            className={`flex flex-col items-center gap-2 p-4 rounded-lg border transition-all ${
              layout === l.id
                ? "border-primary bg-primary/5"
                : "border-line hover:bg-surface"
            }`}
          >
            <span className="text-2xl">{l.icon}</span>
            <span className="text-xs">{l.label}</span>
          </button>
        ))}
      </div>

      <div className="flex justify-between mt-8">
        <Button variant="secondary" onClick={onBack}>
          ← 이전
        </Button>
        <Button onClick={() => onNext({ theme_id: themeId, lobby_mode: layout })} disabled={loading}>
          {loading ? "저장 중..." : "다음 →"}
        </Button>
      </div>
    </div>
  );
}
