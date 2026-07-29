// Dashboard — overview of the selected site

import { useEffect, useState } from "react";
import { useSite, apiGet } from "../shared/api";
import { Card } from "../shared/ui/card";
import { Badge } from "../shared/ui/badge";

interface HealthResponse {
  status: string;
}

interface ExtensionInfo {
  id: string;
  display_name: { ko: string; en: string };
  enabled: boolean;
  purged: boolean;
}

interface Manifest {
  site: { name: string };
  extensions: ExtensionInfo[];
}

export function DashboardPage() {
  const { activeSite } = useSite();
  const [health, setHealth] = useState<string | null>(null);
  const [manifest, setManifest] = useState<Manifest | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    if (!activeSite) {
      setLoading(false);
      return;
    }

    setLoading(true);
    setError(null);

    Promise.all([
      apiGet<HealthResponse>("/healthz")
        .then((r) => setHealth(r.data?.status ?? r.status))
        .catch(() => setHealth(null)),
      apiGet<{ data: Manifest }>("/lobby/manifest")
        .then((r) => setManifest(r.data))
    ])
      .catch((e) => setError(e.message))
      .finally(() => setLoading(false));
  }, [activeSite?.name]);

  if (!activeSite) {
    return (
      <div>
        <h1 className="text-lg font-semibold mb-2">대시보드</h1>
        <p className="text-sm text-muted">
          연결할 사이트가 없습니다. <code>oxipage site add</code>로 사이트를 추가하거나
          sites.toml을 확인하세요.
        </p>
      </div>
    );
  }

  if (loading) {
    return (
      <div>
        <h1 className="text-lg font-semibold mb-2">대시보드</h1>
        <p className="text-sm text-muted">Loading...</p>
      </div>
    );
  }

  const enabledCount = manifest?.extensions?.filter((e) => e.enabled).length ?? 0;
  const totalExt = manifest?.extensions?.length ?? 0;

  return (
    <div>
      <h1 className="text-lg font-semibold mb-1">대시보드</h1>
      <p className="text-xs text-muted mb-6">
        {activeSite.name} &middot; {activeSite.endpoint}
      </p>

      {error && (
        <div className="text-sm text-destructive mb-4 bg-destructive/10 p-3 rounded border border-destructive/20">
          {error}
        </div>
      )}

      <div
        className="grid gap-3 mb-6"
        style={{ gridTemplateColumns: "repeat(auto-fit, minmax(160px, 1fr))" }}
      >
        <div className="stat-card">
          <div
            className={
              health === "ok" ? "stat-value text-positive" : "stat-value text-subtle"
            }
          >
            {health === "ok" ? "\u25CF" : "\u25CB"}
          </div>
          <div className="stat-label">서버 상태</div>
        </div>
        <div className="stat-card">
          <div className="stat-value">{totalExt}</div>
          <div className="stat-label">확장 (활성 {enabledCount})</div>
        </div>
        <div className="stat-card">
          <div className="stat-value">{health === "ok" ? "Online" : "Offline"}</div>
          <div className="stat-label">연결</div>
        </div>
      </div>

      {manifest && manifest.extensions && (
        <Card title="확장 상태" subtitle="활성화된 확장 목록">
          {manifest.extensions.map((ext) => (
            <div
              key={ext.id}
              className="flex items-center justify-between border-b border-line py-1.5"
            >
              <span className="text-sm">{ext.display_name.ko}</span>
              <Badge variant={ext.enabled ? "active" : "inactive"}>
                {ext.enabled ? "활성" : ext.purged ? "Purged" : "비활성"}
              </Badge>
            </div>
          ))}
        </Card>
      )}
    </div>
  );
}
