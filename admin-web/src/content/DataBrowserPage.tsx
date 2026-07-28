// Data browser — generic per-extension record listing

import { useEffect, useState } from "react";
import { useParams } from "react-router";
import { useSite, apiGet, apiPost, apiDelete, type ExtensionInfo } from "../shared/api";
import { Card } from "../shared/ui/card";
import { Button } from "../shared/ui/button";
import { Badge } from "../shared/ui/badge";

interface GenericRecord {
  id?: number | string;
  slug?: string;
  title?: string;
  name?: string;
  published_at?: string | null;
  [key: string]: unknown;
}

interface ExtensionInfoWithData {
  id: string;
  display_name: string;
}

// extensions known to have browseable data
const DATA_EXTENSIONS = [
  "blog", "projects", "links", "novels", "movies", "books", "scraps", "activity",
];

const DISPLAY_NAMES: Record<string, string> = {
  blog: "블로그",
  projects: "프로젝트",
  links: "링크",
  novels: "소설",
  movies: "영화",
  books: "책",
  scraps: "스크랩",
  activity: "활동",
};

export function DataBrowserPage() {
  const { extId } = useParams();
  const { activeSite } = useSite();
  const [records, setRecords] = useState<GenericRecord[]>([]);
  const [extensions, setExtensions] = useState<ExtensionInfoWithData[]>([]);
  const [loading, setLoading] = useState(true);
  const [selectedExt, setSelectedExt] = useState<string>(extId || "blog");

  // Determine the effective extension to show
  const activeExt = extId || selectedExt;

  // Load extension list from server
  useEffect(() => {
    if (!activeSite) return;
    apiGet<{ data: ExtensionInfo[] }>("/extensions")
      .then((res) => {
        const avail = res.data
          .filter((e) => e.enabled && DATA_EXTENSIONS.includes(e.id))
          .map((e) => ({ id: e.id, display_name: e.display_name.ko }));
        setExtensions(avail);
      })
      .catch(() => {});
  }, [activeSite?.name]);

  // Load data for the active extension
  useEffect(() => {
    if (!activeSite || !activeExt) return;
    setLoading(true);
    const path = DATA_EXTENSIONS.includes(activeExt)
      ? `api/v1/${activeExt}`
      : `api/v1/${activeExt}`;
    apiGet<{ data: unknown[] }>(path)
      .then((res) => setRecords(Array.isArray(res.data) ? res.data : []))
      .catch(() => setRecords([]))
      .finally(() => setLoading(false));
  }, [activeSite?.name, activeExt]);

  const handleDelete = async (id: string | number | undefined) => {
    if (!activeSite || !id) return;
    if (!confirm("삭제하시겠습니까?")) return;
    try {
      await apiDelete(`/${activeExt}/${id}`);
      // Refresh
      const res = await apiGet<{ data: unknown[] }>(`/${activeExt}`);
      setRecords(Array.isArray(res.data) ? res.data : []);
    } catch (e: any) {
      alert("Delete failed: " + e.message);
    }
  };

  if (!activeSite) {
    return <div><h1 className="text-lg font-semibold mb-2">데이터 브라우저</h1><p className="text-sm text-[#777]">사이트를 선택하세요.</p></div>;
  }

  // Compute columns from first record
  const sampleRecord = records[0];
  const columns = sampleRecord
    ? Object.keys(sampleRecord).filter((k) => !["id", "body"].includes(k)).slice(0, 6)
    : [];

  const getTitle = (rec: GenericRecord): string => {
    return String(rec.title || rec.name || rec.slug || rec.id || "");
  };

  const getStatus = (rec: GenericRecord): "published" | "draft" | "unknown" => {
    if (rec.published_at) return "published";
    if (rec.published_at === null && "published_at" in rec) return "draft";
    return "unknown";
  };

  const getDeleteId = (rec: GenericRecord): string | number | undefined => {
    return rec.slug ?? rec.id;
  };

  return (
    <div>
      <h1 className="text-lg font-semibold mb-1">데이터 브라우저</h1>
      <p className="text-xs text-[#777] mb-4">{activeSite.name}</p>

      {/* Extension tabs */}
      <div style={{ display: "flex", gap: 6, marginBottom: 16, flexWrap: "wrap" }}>
        {extensions.map((ext) => (
          <button
            key={ext.id}
            onClick={() => setSelectedExt(ext.id)}
            style={{
              padding: "6px 14px",
              borderRadius: 6,
              fontSize: 13,
              border: "1px solid #e8e4e0",
              background: activeExt === ext.id ? "oklch(52% 0.20 290)" : "#fff",
              color: activeExt === ext.id ? "#fff" : "#555",
              cursor: "pointer",
            }}
          >
            {ext.display_name}
          </button>
        ))}
        {extensions.length === 0 && (
          <span className="text-sm text-[#777]">활성화된 확장이 없습니다.</span>
        )}
      </div>

      {loading ? (
        <p className="text-sm text-[#777]">로딩 중...</p>
      ) : records.length === 0 ? (
        <Card>
          <p className="text-sm text-[#777] py-8 text-center">
            데이터가 없습니다.
          </p>
        </Card>
      ) : (
        <Card>
          <table className="admin-table">
            <thead>
              <tr>
                <th>제목/이름</th>
                {columns.slice(0, 4).map((col) => (
                  <th key={col}>{col}</th>
                ))}
                <th>상태</th>
                <th>액션</th>
              </tr>
            </thead>
            <tbody>
              {records.map((rec, idx) => (
                <tr key={rec.slug as string || rec.id as number || idx}>
                  <td className="font-medium">{getTitle(rec).slice(0, 50)}</td>
                  {columns.slice(0, 4).map((col) => (
                    <td key={col} className="text-xs text-[#777]">
                      {typeof rec[col] === "string"
                        ? (rec[col] as string).slice(0, 30)
                        : typeof rec[col] === "object"
                        ? JSON.stringify(rec[col]).slice(0, 20)
                        : String(rec[col] ?? "")}
                    </td>
                  ))}
                  <td>
                    {getStatus(rec) !== "unknown" && (
                      <Badge variant={getStatus(rec) === "published" ? "active" : "inactive"}>
                        {getStatus(rec) === "published" ? "발행" : "초안"}
                      </Badge>
                    )}
                    {getStatus(rec) === "unknown" && <span className="text-xs text-[#777]">—</span>}
                  </td>
                  <td>
                    <Button
                      size="sm"
                      variant="destructive"
                      onClick={() => handleDelete(getDeleteId(rec))}
                    >
                      삭제
                    </Button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </Card>
      )}
    </div>
  );
}
