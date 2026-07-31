import { useEffect } from "react";
import { Outlet, useParams } from "react-router";
import { applyServerTheme } from "../../shared/theme";
import { Topbar } from "./Topbar";
import { Sidebar } from "./Sidebar";
import { OfflineBanner } from "../shared/ui/OfflineBanner";

export function ConsoleShell() {
  const { slug } = useParams();
  useEffect(() => {
    void applyServerTheme(slug);
  }, [slug]);
  return (
    <div className="min-h-screen bg-canvas flex flex-col">
      <Topbar />
      <OfflineBanner />
      <div className="flex flex-1">
        <Sidebar />
        <main className="flex-1 p-6 overflow-auto">
          <Outlet />
        </main>
      </div>
    </div>
  );
}
