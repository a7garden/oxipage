// 테마 선택 페이지 — 카드 그리드 + 미리보기 + 원클릭 적용

import { useEffect, useState } from "react";
import { useSite, getThemeCatalog, sitePut, type ThemeInfo } from "../shared/api";
import { Card } from "../shared/ui/card";
import { Button } from "../shared/ui/button";

export function ThemesPage() {
  const { activeSite } = useSite();
  const [themes, setThemes] = useState<ThemeInfo[]>([]);
  const [currentTheme, setCurrentTheme] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [applying, setApplying] = useState(false);

  const fetchData = async () => {
    setLoading(true);
    try {
      const [catRes, currentRes] = await Promise.all([
        getThemeCatalog(),
        activeSite
          ? (fetch(`/api/admin/proxy/${activeSite.name}/api/v1/theme`).then((r) =>
              r.json(),
            ) as Promise<{ data: { theme_id: string } }>)
          : Promise.resolve({ data: { theme_id: "paper" } }),
      ]);
      setThemes(catRes.data);
      setCurrentTheme(currentRes.data.theme_id);
    } catch {
      // Fall back to catalog only
      const catRes = await getThemeCatalog();
      setThemes(catRes.data);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchData();
  }, [activeSite?.name]);

  const applyTheme = async (id: string) => {
    if (!activeSite) return;
    setApplying(true);
    try {
      await sitePut(activeSite.name, "api/v1/theme", { theme_id: id });
      setCurrentTheme(id);
    } catch (e: any) {
      alert("Failed to apply theme: " + e.message);
    } finally {
      setApplying(false);
    }
  };

  return (
    <div>
      <h1 className="text-lg font-semibold mb-1">블로그 테마</h1>
      <p className="text-xs text-[#777] mb-6">
        {activeSite ? activeSite.name : "No site selected"}
      </p>

      {loading ? (
        <p className="text-sm text-[#777]">로딩 중...</p>
      ) : (
        <div
          style={{
            display: "grid",
            gridTemplateColumns: "repeat(auto-fill, minmax(240px, 1fr))",
            gap: 16,
          }}
        >
          {themes.map((theme) => {
            const isActive = currentTheme === theme.id;
            return (
              <div
                key={theme.id}
                className={`theme-card ${isActive ? "active" : ""}`}
                onClick={() => applyTheme(theme.id)}
              >
                {/* Preview swatch */}
                <div className="preview-swatch">
                  {theme.preview_colors.slice(0, 4).map((color, i) => (
                    <div key={i} style={{ background: color }} />
                  ))}
                </div>

                {/* Info */}
                <div style={{ padding: "10px 12px" }}>
                  <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                    <strong className="text-sm">{theme.name_ko}</strong>
                    {isActive && (
                      <span className="text-xs text-[oklch(50%_0.15_145)] font-medium">
                        적용됨
                      </span>
                    )}
                  </div>
                  <p className="text-xs text-[#777] mt-1">
                    {theme.description_ko}
                  </p>
                  <div style={{ marginTop: 8 }}>
                    <span className="text-xs text-[#999]">
                      {theme.mode === "light" ? "라이트" : "다크"} &middot; 악센트 {theme.accent_hue}°
                    </span>
                  </div>
                  {!isActive && (
                    <div style={{ marginTop: 8 }}>
                      <Button
                        size="sm"
                        variant="outline"
                        disabled={applying}
                        onClick={(e: React.MouseEvent) => {
                          e.stopPropagation();
                          applyTheme(theme.id);
                        }}
                      >
                        {applying ? "적용 중..." : "적용"}
                      </Button>
                    </div>
                  )}
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
