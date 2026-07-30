import { useState } from "react";
import { useParams } from "react-router";
import { Button } from "../../shared/ui/button";

const themes = [
  {
    id: "paper",
    name: "Paper",
    light: "oklch(98.5% 0.004 95)",
    dark: "oklch(13% 0.020 265)",
    accent: "#22c55e",
    body: "oklch(75% 0.010 95)",
  },
  {
    id: "midnight",
    name: "Midnight",
    light: "oklch(10% 0.025 265)",
    dark: "oklch(96% 0.005 250)",
    accent: "#4ade80",
    body: "oklch(35% 0.012 265)",
  },
  {
    id: "sepia",
    name: "Sepia",
    light: "oklch(96% 0.02 80)",
    dark: "oklch(15% 0.015 60)",
    accent: "#eab308",
    body: "oklch(82% 0.15 85)",
  },
  {
    id: "forest",
    name: "Forest",
    light: "oklch(97% 0.01 145)",
    dark: "oklch(12% 0.02 155)",
    accent: "#22c55e",
    body: "oklch(75% 0.010 145)",
  },
];

type Theme = (typeof themes)[0];

function ThemePreview({ theme }: { theme: Theme }) {
  return (
    <div className="h-20 p-3 rounded-t-lg" style={{ background: theme.light }}>
      <div className="text-xs font-bold mb-1.5" style={{ color: theme.dark }}>
        {theme.name}
      </div>
      <div className="h-1.5 rounded-sm mb-1" style={{ width: "70%", background: theme.accent }} />
      <div className="h-1.5 rounded-sm mb-1" style={{ width: "50%", background: theme.body }} />
      <div className="h-1.5 rounded-sm" style={{ width: "30%", background: theme.body }} />
    </div>
  );
}

export function ThemesPage() {
  const { slug } = useParams<{ slug: string }>()!;
  const [current, setCurrent] = useState("paper");

  return (
    <div>
      <h1 className="text-xl font-bold text-foreground mb-1">Themes</h1>
      <p className="text-sm text-muted mb-6">Pick a visual theme for the public site</p>

      <div className="grid grid-cols-4 gap-3 mb-6">
        {themes.map((theme) => (
          <button
            key={theme.id}
            onClick={() => setCurrent(theme.id)}
            className={`border rounded-lg overflow-hidden text-left cursor-pointer transition-all ${
              current === theme.id ? "border-[#22c55e] border-2" : "border-line hover:border-[#22c55e]"
            }`}
          >
            <ThemePreview theme={theme} />
            <div className="px-3 py-2 border-t border-line flex items-center justify-between">
              <span className="text-sm font-medium">{theme.name}</span>
              {current === theme.id && (
                <span className="text-xs font-bold text-[#22c55e]">✓ Current</span>
              )}
            </div>
          </button>
        ))}
      </div>

      {/* Preview */}
      <div className="border border-line rounded-lg p-5 bg-surface/30">
        <div className="text-xs font-semibold text-muted uppercase tracking-wider mb-3">
          Preview — {slug} landing page
        </div>
        <div className="flex items-center justify-between mb-4">
          <div className="font-display text-lg font-bold">My Blog</div>
          <div className="text-xs text-muted">🌙 · ko</div>
        </div>
        <div className="grid grid-cols-3 gap-3">
          {[
            { title: "Console Redesign", excerpt: "A new era for Oxipage management..." },
            { title: "WASM v2 Benchmarks", excerpt: "Performance numbers..." },
            { title: "멀티사이트 가이드", excerpt: "여러 블로그를 한 번에 관리하는 방법..." },
          ].map((item) => (
            <div key={item.title} className="border border-line rounded-md p-3 bg-canvas">
              <div className="text-sm font-semibold mb-1">{item.title}</div>
              <div className="text-xs text-muted">{item.excerpt}</div>
            </div>
          ))}
        </div>
      </div>

      <div className="flex justify-end mt-4">
        <Button
          onClick={() =>
            fetch(`/api/console/s/${slug}/theme`, {
              method: "PUT",
              headers: { "content-type": "application/json" },
              body: JSON.stringify({ theme_id: current }),
            })
          }
        >
          Apply Theme
        </Button>
      </div>
    </div>
  );
}
