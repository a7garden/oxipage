import { BrowserRouter, Routes, Route, Outlet } from "react-router";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { SiteShell } from "./shell/SiteShell";
import { HomeRedirect } from "./sites/HomeRedirect";
import { SitesPage } from "./sites/SitesPage";

const queryClient = new QueryClient();

function DashboardPage() {
  return <div>Dashboard (stub)</div>;
}
function NewSiteWizardPage() {
  return <div>New Site Wizard (stub)</div>;
}

function AdminShell() {
  return (
    <SiteShell>
      <Outlet />
    </SiteShell>
  );
}

export function AdminApp() {
  return (
    <QueryClientProvider client={queryClient}>
      <BrowserRouter>
        <Routes>
          <Route path="/" element={<AdminShell />}>
            <Route index element={<HomeRedirect />} />
            <Route path="sites" element={<SitesPage />} />
            <Route path="sites/new" element={<NewSiteWizardPage />} />
            <Route path="s/:slug" element={<DashboardPage />} />
          </Route>
        </Routes>
      </BrowserRouter>
    </QueryClientProvider>
  );
}
