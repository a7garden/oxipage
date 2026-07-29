// Settings page — site profile management + site details

import { useEffect, useState } from "react";
import { useSite, listSites, addSite, deleteSite, setActiveSite } from "../shared/api";
import { Card } from "../shared/ui/card";
import { Button } from "../shared/ui/button";
import { Badge } from "../shared/ui/badge";
import { Input } from "../shared/ui/input";

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
        <Card title="현재 사이트" subtitle={activeSite.endpoint} className="mb-4">
          <div className="flex items-center gap-2 mt-1">
            <Badge variant="active">활성</Badge>
            <span className="text-sm font-medium">{activeSite.name}</span>
            <span className="text-xs text-muted">{activeSite.endpoint}</span>
          </div>
        </Card>
      )}

      {error && (
        <div className="text-sm text-destructive mb-4 bg-destructive/10 p-3 rounded border border-destructive/20">{error}</div>
      )}

      {/* Site list */}
      <Card title="등록된 사이트" className="mb-4">
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
                  {site.active && <Badge variant="active" className="ml-1.5">활성</Badge>}
                </td>
                <td className="text-xs text-muted font-mono">{site.endpoint}</td>
                <td className="text-xs text-muted">{site.token_masked || "없음"}</td>
                <td>
                  <div className="flex gap-1">
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
          <div className="mt-3 p-3 border border-line rounded-md">
            <div className="flex flex-col gap-2">
              <Input
                type="text" placeholder="사이트 이름" value={form.name}
                onChange={(e) => setForm({ ...form, name: e.target.value })}
              />
              <Input
                type="text" placeholder="엔드포인트 (http://...)"
                value={form.endpoint}
                onChange={(e) => setForm({ ...form, endpoint: e.target.value })}
              />
              <Input
                type="text" placeholder="토큰 (옵션)"
                value={form.token}
                onChange={(e) => setForm({ ...form, token: e.target.value })}
              />
              <div className="flex gap-1.5">
                <Button size="sm" onClick={handleAdd}>추가</Button>
                <Button size="sm" variant="outline" onClick={() => { setShowForm(false); setForm(emptyForm); setError(null); }}>취소</Button>
              </div>
            </div>
          </div>
        ) : (
          <div className="mt-3">
            <Button size="sm" variant="primary" onClick={() => setShowForm(true)}>
              + 사이트 추가
            </Button>
          </div>
        )}
      </Card>
    </div>
  );
}
