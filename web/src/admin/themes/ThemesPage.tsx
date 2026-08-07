import { useState, useEffect } from "react";
import { useParams } from "react-router";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { getTheme, setTheme, listThemes } from "../shared/api";
import { applyServerTheme, type ThemeDefinition } from "../../shared/theme";
import { Button } from "../../shared/ui/button";
import { Skeleton } from "../../shared/ui/skeleton";
import { ConsolePageHeader } from "../shell/ConsolePageHeader";

function ThemePreview({ theme }: { theme: ThemeDefinition }) {
  const [bg, _body, text, accent] = theme.preview_colors;
  return (
    <div className="h-20 p-3 rounded-t-lg" style={{ background: bg }}>
      <div className="text-xs font-mono mb-1" style={{ color: text }}>Aa 가나다</div>
      <div className="h-1.5 w-12 rounded-full" style={{ background: accent, opacity: 0.9 }} />
    </div>
  );
}

export function ThemesPage() {
  const { slug } = useParams<{ slug: string }>()!;
  const qc = useQueryClient();
  const [current, setCurrent] = useState("paper");
  const [layout, setLayout] = useState("shell");

  const { data, isLoading, isError } = useQuery({
    queryKey: ["site", slug, "theme"],
    queryFn: () => getTheme(slug!),
    enabled: !!slug,
  });
  const { data: catalog = [] } = useQuery<ThemeDefinition[]>({
    queryKey: ["console", "themes"],
    queryFn: listThemes,
  });

  useEffect(() => {
    if (data?.theme_id) setCurrent(data.theme_id);
    if (data?.layout) setLayout(data.layout);
  }, [data]);

  const apply = useMutation({
    mutationFn: () => setTheme(slug!, current, layout),
    onSuccess: (next) => {
      qc.setQueryData(["site", slug, "theme"], next);
      void applyServerTheme(slug!);
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
      <ConsolePageHeader
        title="Themes"
        description="Pick a visual theme for the public site"
        border={false}
      />

      <div className="grid grid-cols-4 gap-3 mb-6">
        {catalog.map((theme) => (
          <button
            key={theme.id}
            onClick={() => setCurrent(theme.id)}
            className={`border rounded-lg overflow-hidden text-left cursor-pointer transition-all ${
              current === theme.id ? "border-primary border-2" : "border-line hover:border-primary"
            }`}
          >
            <ThemePreview theme={theme} />
            <div className="px-3 py-2 border-t border-line flex items-center justify-between">
              <span className="text-sm font-medium">{theme.name_en}</span>
              {current === theme.id && (
                <span className="text-xs font-bold text-primary">✓ Current</span>
              )}
            </div>
          </button>
        ))}
      </div>

      <h2 className="text-sm font-semibold text-muted uppercase tracking-wider mb-2">Layout</h2>
      <div className="grid grid-cols-2 gap-3 mb-6">
        {[
          { id: "shell", name: "Shell", desc: "Sticky header · nav · footer" },
          { id: "editorial", name: "Editorial", desc: "Hub lobby · no chrome" },
        ].map((l) => (
          <button
            key={l.id}
            onClick={() => setLayout(l.id)}
            className={`border rounded-lg p-3 text-left cursor-pointer transition-all ${
              layout === l.id ? "border-primary border-2" : "border-line hover:border-primary"
            }`}
          >
            <div className="text-sm font-medium">{l.name}</div>
            <div className="text-xs text-muted">{l.desc}</div>
            {layout === l.id && (
              <span className="text-xs font-bold text-primary">✓ Current</span>
            )}
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
            { title: "Console Redesign", excerpt: "A new era for Oxibuilder management..." },
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
          <span className="text-sm text-destructive-fg">
            {apply.error instanceof Error ? apply.error.message : "Apply failed"}
          </span>
        )}
        {apply.isSuccess && (
          <span className="text-sm text-positive-fg">Theme applied</span>
        )}
        <Button
          onClick={() => apply.mutate()}
          disabled={apply.isPending || (current === data?.theme_id && layout === data?.layout)}
        >
          {apply.isPending ? "Applying..." : "Apply Theme"}
        </Button>
      </div>
    </div>
  );
}
