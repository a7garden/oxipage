import { Outlet } from "react-router";
import { Topbar } from "./Topbar";
import { Sidebar } from "./Sidebar";
import { OfflineBanner } from "../shared/ui/OfflineBanner";

export function ConsoleShell() {
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
