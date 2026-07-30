import { useQuery } from "@tanstack/react-query";
import { Navigate } from "react-router";
import { listSites } from "../shared/api";

export function HomeRedirect() {
  const { data } = useQuery({ queryKey: ["sites"], queryFn: listSites });
  const sites = data?.data ?? [];

  if (sites.length === 0) {
    return <Navigate to="/sites" replace />;
  }

  const defaultSlug = sites.find((s) => s.active)?.name ?? sites[0].name;
  return <Navigate to={`/s/${defaultSlug}`} replace />;
}
