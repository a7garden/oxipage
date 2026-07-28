// Settings page — site profile management + site details

import { useEffect, useState } from "react";
import { useSite, listSites, addSite, deleteSite, setActiveSite } from "../shared/api";
import { Card } from "../shared/ui/card";
import { Button } from "../shared/ui/button";
import { Badge } from "../shared/ui/badge";

interface SiteForm {
  name: string;
  endpoint: string;
  token: string;
}

const emptyForm: SiteForm = { name: "", endpoint: "", token: "" };

export function SettingsPage() {
  const { activeSite, sites, refreshSites, setActiveSite: switchSite } = useSite();
  const [showForm, setShowForm] = useState(false);
  const [form, setForm] = useState<SiteForm>(emptyForm);
  const [error, setError] = useState<string | null>(null);

  const handleAdd = async () => {
    if (!form.name.trim() || !form.endpoint.trim()) {
      setError("Name and endpoint are required.");
      return;
    }
    setError(null);
    try {
      await addSite(form.name.trim(), form.endpoint.trim(), form.token.trim() || undefined);
      await refreshSites();
      setShowForm(false);
      setForm(emptyForm);
    } catch (e: any) {
      setError(e.message);
    }
  };

  const handleDelete = async (name: string) => {
    if (!confirm(`"${name}" 사이트를 삭제하시겠습니까?`)) return;
    try {
      await deleteSite(name);
      await refreshSites();
    } catch (e: any) {
      alert(e.message);
    }
  };

  const handleSetActive = async (name: string) => {
    await switchSite(name);
  };

  return (
    <div>
      <h1 className="text-lg font-semibold mb-4">설정</h1>

      {/* Active site info */}
      {activeSite && (
        <Card title="현재 사이트" subtitle={activeSite.endpoint} style={{ marginBottom: 16 }}>
          <div style={{ display: "flex", alignItems: "center", gap: 8, marginTop: 4 }}>
            <Badge variant="active">활성</Badge>
            <span className="text-sm font-medium">{activeSite.name}</span>
            <span className="text-xs text-[#777]">{activeSite.endpoint}</span>
          </div>
        </Card>
      )}

      {error && (
        <div className="text-sm text-red-600 mb-4 bg-red-50 p-3 rounded border border-red-200">{error}</div>
      )}

      {/* Site list */}
      <Card title="등록된 사이트" style={{ marginBottom: 16 }}>
        <table className="admin-table">
          <thead>
            <tr>
              <th>이름</th>
              <th>엔드포인트</th>
              <th>토큰</th>
              <th>액션</th>
            </tr>
          </thead>
          <tbody>
            {sites.map((site) => (
              <tr key={site.name}>
                <td className="font-medium">
                  {site.name}
                  {site.active && <Badge variant="active" style={{ marginLeft: 6 }}>활성</Badge>}
                </td>
                <td className="text-xs text-[#777] font-mono">{site.endpoint}</td>
                <td className="text-xs text-[#777]">{site.token_masked || "없음"}</td>
                <td>
                  <div style={{ display: "flex", gap: 4 }}>
                    {!site.active && (
                      <Button size="sm" variant="outline" onClick={() => handleSetActive(site.name)}>
                        활성화
                      </Button>
                    )}
                    <Button size="sm" variant="destructive" onClick={() => handleDelete(site.name)}>
                      삭제
                    </Button>
                  </div>
                </td>
              </tr>
            ))}
          </tbody>
        </table>

        {showForm ? (
          <div style={{ marginTop: 12, padding: 12, border: "1px solid #e8e4e0", borderRadius: 6 }}>
            <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
              <input
                type="text" placeholder="사이트 이름" value={form.name}
                onChange={(e) => setForm({ ...form, name: e.target.value })}
                style={{ padding: "8px 10px", border: "1px solid #e8e4e0", borderRadius: 4, fontSize: 13, outline: "none" }}
              />
              <input
                type="text" placeholder="엔드포인트 (http://...)"
                value={form.endpoint}
                onChange={(e) => setForm({ ...form, endpoint: e.target.value })}
                style={{ padding: "8px 10px", border: "1px solid #e8e4e0", borderRadius: 4, fontSize: 13, outline: "none" }}
              />
              <input
                type="text" placeholder="토큰 (옵션)"
                value={form.token}
                onChange={(e) => setForm({ ...form, token: e.target.value })}
                style={{ padding: "8px 10px", border: "1px solid #e8e4e0", borderRadius: 4, fontSize: 13, outline: "none" }}
              />
              <div style={{ display: "flex", gap: 6 }}>
                <Button size="sm" onClick={handleAdd}>추가</Button>
                <Button size="sm" variant="outline" onClick={() => { setShowForm(false); setForm(emptyForm); setError(null); }}>취소</Button>
              </div>
            </div>
          </div>
        ) : (
          <div style={{ marginTop: 12 }}>
            <Button size="sm" variant="primary" onClick={() => setShowForm(true)}>
              + 사이트 추가
            </Button>
          </div>
        )}
      </Card>
    </div>
  );
}
