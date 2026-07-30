import { useState, useEffect } from "react";
import { useParams } from "react-router";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { getTheme, setTheme } from "../shared/api";
import { Button } from "../../shared/ui/button";
import { Skeleton } from "../../shared/ui/skeleton";

interface Theme {
  id: string;
  name: string;
  light: string;
  dark: string;
  accent: string;
  body: string;
}

const THEMES: Theme[] = [
  { id: "paper", name: "Paper", light: "oklch(98.5% 0.004 95)", dark: "oklch(13% 0.020 265)", accent: "#22c55e", body: "oklch(75% 0.010 95)" },
  { id: "midnight", name: "Midnight", light: "oklch(10% 0.025 265)", dark: "oklch(96% 0.005 250)", accent: "#4ade80", body: "oklch(35% 0.012 265)" },
  { id: "sepia", name: "Sepia", light: "oklch(96% 0.02 80)", dark: "oklch(15% 0.015 60)", accent: "#eab308", body: "oklch(82% 0.15 85)" },
  { id: "neon", name: "Neon", light: "oklch(96% 0.005 280)", dark: "oklch(8% 0.030 290)", accent: "#a855f7", body: "oklch(60% 0.20 290)" },
  { id: "canvas", name: "Canvas", light: "oklch(99% 0.003 95)", dark: "oklch(18% 0.015 265)", accent: "#0ea5e9", body: "oklch(70% 0.020 240)" },
];

function ThemePreview({ theme }: { theme: Theme }) {
  return (
    <div className="h-20 p-3 rounded-t-lg" style={{ background: theme.light }}>
      <div className="text-xs font-mono mb-1" style={{ color: theme.body }}>Aa 가나다</div>
      <div className="h-1.5 w-12 rounded-full" style={{ background: theme.accent }} />
    </div>
  );
}

export function ThemesPage() {
  const { slug } = useParams<{ slug: string }>()!;
  const qc = useQueryClient();
  const [current, setCurrent] = useState("paper");

  const { data, isLoading, isError } = useQuery({
    queryKey: ["site", slug, "theme"],
    queryFn: () => getTheme(slug!),
    enabled: !!slug,
  });

  useEffect(() => {
    if (data?.theme_id) setCurrent(data.theme_id);
  }, [data]);

  const apply = useMutation({
    mutationFn: () => setTheme(slug!, current),
    onSuccess: (next) => {
      qc.setQueryData(["site", slug, "theme"], next);
    },
  });

  if (isLoading) {
    return (
      <div className="space-y-4">
        <Skeleton className="h-8 w-48" />
        <Skeleton className="h-32 w-full" />
      </div>
    );
  }

  if (isError) {
    return (
      <div className="border border-line rounded-lg p-6 text-center text-muted text-sm">
        Failed to load theme.{" "}
        <button onClick={() => qc.invalidateQueries({ queryKey: ["site", slug, "theme"] })} className="underline">Retry</button>
      </div>
    );
  }

  return (
    <div>
      <h1 className="text-xl font-bold text-foreground mb-1">Themes</h1>
      <p className="text-sm text-muted mb-6">Pick a visual theme for the public site</p>

      <div className="grid grid-cols-4 gap-3 mb-6">
        {THEMES.map((theme) => (
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

      <div className="flex justify-end items-center gap-3 mt-4">
        {apply.isError && (
          <span className="text-sm text-red-600">
            {apply.error instanceof Error ? apply.error.message : "Apply failed"}
          </span>
        )}
        {apply.isSuccess && (
          <span className="text-sm text-[#22c55e]">Theme applied</span>
        )}
        <Button
          onClick={() => apply.mutate()}
          disabled={apply.isPending || current === data?.theme_id}
        >
          {apply.isPending ? "Applying..." : "Apply Theme"}
        </Button>
      </div>
    </div>
  );
}
