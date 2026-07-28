// PAT management — list, create, revoke tokens

import { useEffect, useState } from "react";
import { useSite, listTokens, createToken, revokeToken, type PatRow } from "../shared/api";
import { Card } from "../shared/ui/card";
import { Button } from "../shared/ui/button";
import { Badge } from "../shared/ui/badge";

export function TokensPage() {
  const { activeSite } = useSite();
  const [tokens, setTokens] = useState<PatRow[]>([]);
  const [loading, setLoading] = useState(true);
  const [showCreate, setShowCreate] = useState(false);
  const [newLabel, setNewLabel] = useState("");
  const [newToken, setNewToken] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const fetchTokens = async () => {
    if (!activeSite) return;
    setLoading(true);
    try {
      const res = await listTokens();
      setTokens(res.data);
    } catch (e: any) {
      setError(e.message);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchTokens();
  }, [activeSite?.name]);

  const handleCreate = async () => {
    if (!activeSite || !newLabel.trim()) return;
    setError(null);
    try {
      const res = await createToken(newLabel.trim(), "admin");
      setNewToken(res.data.token);
      setNewLabel("");
      setShowCreate(false);
      fetchTokens();
    } catch (e: any) {
      setError(e.message);
    }
  };

  const handleRevoke = async (id: number) => {
    if (!activeSite || !confirm("토큰을 폐기하시겠습니까? 되돌릴 수 없습니다.")) return;
    try {
      await revokeToken(id);
      fetchTokens();
    } catch (e: any) {
      alert(e.message);
    }
  };

  if (!activeSite) {
    return <div><h1 className="text-lg font-semibold mb-2">토큰</h1><p className="text-sm text-[#777]">사이트를 선택하세요.</p></div>;
  }

  return (
    <div>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 16 }}>
        <div>
          <h1 className="text-lg font-semibold">API 토큰 관리</h1>
          <p className="text-xs text-[#777]">{activeSite.name}</p>
        </div>
        <Button onClick={() => { setShowCreate(true); setNewToken(null); }}>새 토큰</Button>
      </div>

      {error && (
        <div className="text-sm text-red-600 mb-4 bg-red-50 p-3 rounded border border-red-200">{error}</div>
      )}

      {/* Newly created token — show once */}
      {newToken && (
        <Card title="토큰 생성 완료" style={{ marginBottom: 16, border: "2px solid oklch(60% 0.15 145)" }}>
          <p className="text-sm mb-2">이 토큰은 다시 표시되지 않습니다. 안전한 곳에 저장하세요.</p>
          <div className="text-sm font-mono bg-[#f5f2ed] p-3 rounded select-all break-all">{newToken}</div>
          <div style={{ marginTop: 8 }}>
            <Button size="sm" variant="outline" onClick={() => setNewToken(null)}>확인</Button>
          </div>
        </Card>
      )}

      {/* Create form */}
      {showCreate && !newToken && (
        <Card style={{ marginBottom: 16 }}>
          <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
            <input
              type="text"
              placeholder="토큰 레이블 (예: omp-agent)"
              value={newLabel}
              onChange={(e) => setNewLabel(e.target.value)}
              autoFocus
              style={{ flex: 1, padding: "8px 10px", border: "1px solid #e8e4e0", borderRadius: 4, fontSize: 13, outline: "none" }}
            />
            <Button size="sm" onClick={handleCreate} disabled={!newLabel.trim()}>생성</Button>
            <Button size="sm" variant="outline" onClick={() => { setShowCreate(false); setNewLabel(""); }}>취소</Button>
          </div>
        </Card>
      )}

      {loading ? (
        <p className="text-sm text-[#777]">로딩 중...</p>
      ) : (
        <Card>
          <table className="admin-table">
            <thead>
              <tr>
                <th>레이블</th>
                <th>스코프</th>
                <th>생성일</th>
                <th>만료일</th>
                <th>액션</th>
              </tr>
            </thead>
            <tbody>
              {tokens.map((tok) => (
                <tr key={tok.id}>
                  <td className="font-medium">{tok.label}</td>
                  <td><span className="text-xs text-[#777]">{tok.scopes}</span></td>
                  <td className="text-xs text-[#777]">{tok.created_at.slice(0, 10)}</td>
                  <td className="text-xs text-[#777]">{tok.expires_at?.slice(0, 10) || "없음"}</td>
                  <td>
                    <Button size="sm" variant="destructive" onClick={() => handleRevoke(tok.id)}>폐기</Button>
                  </td>
                </tr>
              ))}
              {tokens.length === 0 && (
                <tr>
                  <td colSpan={5} className="text-center text-sm text-[#777] py-8">
                    등록된 토큰이 없습니다.
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </Card>
      )}
    </div>
  );
}
