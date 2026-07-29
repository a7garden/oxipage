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
        <p className="text-sm text-muted">연결할 사이트를 선택하세요.</p>
      </div>
    );
  }

  if (loading) {
    return (
      <div>
        <h1 className="text-lg font-semibold mb-2">확장 관리</h1>
        <p className="text-sm text-muted">로딩 중...</p>
      </div>
    );
  }

  return (
    <div>
      <h1 className="text-lg font-semibold mb-1">확장 관리</h1>
      <p className="text-xs text-muted mb-6">{activeSite.name}</p>

      {error && (
        <div className="text-sm text-destructive mb-4 bg-destructive/10 p-3 rounded border border-destructive/20">
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
                <td className="text-xs text-muted font-mono">{ext.id}</td>
                <td>
                  <span
                    className={`inline-flex items-center gap-1 text-xs font-medium ${
                      ext.enabled ? "text-positive" : ext.purged ? "text-destructive" : "text-muted"
                    }`}
                  >
                    <span className="inline-block size-1.5 rounded-full bg-current" />
                    {ext.purged ? "Purged" : ext.enabled ? "활성" : "비활성"}
                  </span>
                </td>
                <td>
                  <div className="flex gap-1.5">
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
                <td colSpan={4} className="text-center text-sm text-muted py-8">
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
