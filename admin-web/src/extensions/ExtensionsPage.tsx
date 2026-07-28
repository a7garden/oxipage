// 확장 관리 페이지 — 활성/비활성 토글, purge, 목록 조회

import { useEffect, useState } from "react";
import { useSite, apiGet, apiPost, apiDelete } from "../shared/api";
import { Card } from "../shared/ui/card";
import { Button } from "../shared/ui/button";

interface ExtensionInfo {
  id: string;
  display_name: { ko: string; en: string };
  enabled: boolean;
  purged: boolean;
}

export function ExtensionsPage() {
  const { activeSite } = useSite();
  const [extensions, setExtensions] = useState<ExtensionInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchExtensions = async () => {
    if (!activeSite) return;
    setLoading(true);
    setError(null);
    try {
      const res = await apiGet<{ data: ExtensionInfo[] }>("api/v1/extensions",
      );
      setExtensions(res.data);
    } catch (e: any) {
      setError(e.message);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchExtensions();
  }, [activeSite?.name]);

  const toggleExtension = async (id: string, enable: boolean) => {
    if (!activeSite) return;
    const path = enable
      ? `api/v1/extensions/${id}/enable`
      : `api/v1/extensions/${id}/disable`;
    await apiPost(path);
    await fetchExtensions();
  };

  const purgeExtension = async (id: string) => {
    if (!activeSite) return;
    if (!confirm(`정말 ${id} 확장을 purge하시겠습니까? 데이터가 삭제됩니다.`)) return;
    await apiDelete(`/extensions/${id}`);
    await fetchExtensions();
  };

  if (!activeSite) {
    return (
      <div>
        <h1 className="text-lg font-semibold mb-2">확장 관리</h1>
        <p className="text-sm text-[#777]">연결할 사이트를 선택하세요.</p>
      </div>
    );
  }

  if (loading) {
    return (
      <div>
        <h1 className="text-lg font-semibold mb-2">확장 관리</h1>
        <p className="text-sm text-[#777]">로딩 중...</p>
      </div>
    );
  }

  return (
    <div>
      <h1 className="text-lg font-semibold mb-1">확장 관리</h1>
      <p className="text-xs text-[#777] mb-6">{activeSite.name}</p>

      {error && (
        <div className="text-sm text-red-600 mb-4 bg-red-50 p-3 rounded border border-red-200">
          {error}
        </div>
      )}

      <Card>
        <table className="admin-table">
          <thead>
            <tr>
              <th>확장</th>
              <th>ID</th>
              <th>상태</th>
              <th>액션</th>
            </tr>
          </thead>
          <tbody>
            {extensions.map((ext) => (
              <tr key={ext.id}>
                <td className="font-medium">{ext.display_name.ko}</td>
                <td className="text-xs text-[#777] font-mono">{ext.id}</td>
                <td>
                  <span
                    className={`inline-flex items-center gap-1 text-xs font-medium ${
                      ext.enabled ? "text-[oklch(50%_0.15_145)]" : ext.purged ? "text-red-600" : "text-[#888]"
                    }`}
                  >
                    <span
                      style={{
                        display: "inline-block",
                        width: 6,
                        height: 6,
                        borderRadius: "50%",
                        background: ext.enabled
                          ? "oklch(60% 0.15 145)"
                          : ext.purged
                          ? "oklch(55% 0.19 25)"
                          : "#ccc",
                      }}
                    />
                    {ext.purged ? "Purged" : ext.enabled ? "활성" : "비활성"}
                  </span>
                </td>
                <td>
                  <div style={{ display: "flex", gap: 6 }}>
                    {ext.enabled ? (
                      <Button size="sm" variant="outline"
                        onClick={() => toggleExtension(ext.id, false)}
                      >
                        비활성화
                      </Button>
                    ) : (
                      <Button size="sm" variant="primary"
                        onClick={() => toggleExtension(ext.id, true)}
                        disabled={ext.purged}
                      >
                        활성화
                      </Button>
                    )}
                    <Button size="sm" variant="destructive"
                      onClick={() => purgeExtension(ext.id)}
                    >
                      Purge
                    </Button>
                  </div>
                </td>
              </tr>
            ))}
            {extensions.length === 0 && (
              <tr>
                <td colSpan={4} className="text-center text-sm text-[#777] py-8">
                  확장이 없습니다.
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </Card>
    </div>
  );
}
