import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { Link } from "react-router";
import { listSites, removeSite, setDefaultSite } from "../shared/api";

export function SitesPage() {
  const qc = useQueryClient();
  const { data } = useQuery({ queryKey: ["sites"], queryFn: listSites });
  const remove = useMutation({
    mutationFn: removeSite,
    onSuccess: () => qc.invalidateQueries({ queryKey: ["sites"] }),
  });
  const setDef = useMutation({
    mutationFn: setDefaultSite,
    onSuccess: () => qc.invalidateQueries({ queryKey: ["sites"] }),
  });

  const sites = data?.data ?? [];

  return (
    <div className="sites-page">
      <h1>사이트 관리</h1>
      {sites.length === 0 && <p>등록된 사이트가 없습니다.</p>}
      <ul>
        {sites.map((s) => (
          <li key={s.name}>
            <Link to={`/s/${s.name}`}>{s.name}</Link>
            <span>{s.path}</span>
            {s.active && <span className="active-badge">활성</span>}
            <button onClick={() => setDef.mutate(s.name)}>기본으로 설정</button>
            <button onClick={() => remove.mutate(s.name)}>삭제</button>
          </li>
        ))}
      </ul>
      <Link to="/sites/new">새 사이트 추가</Link>
    </div>
  );
}
