import { lazy, Suspense } from "react";
import { BrowserRouter, Routes, Route, Outlet } from "react-router";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { SiteShell } from "./shell/SiteShell";
import { HomeRedirect } from "./sites/HomeRedirect";
import { SitesPage } from "./sites/SitesPage";
import { NewSiteWizardPage } from "./sites/NewSiteWizardPage";
import { SetupGuard } from "../setup/SetupGuard";
import { Skeleton } from "../shared/ui/skeleton";

const SetupWizard = lazy(() =>
  import("../setup/SetupWizard").then((m) => ({ default: m.SetupWizard })),
);

const queryClient = new QueryClient();

function DashboardPage() {
  return (
    <div className="flex items-center justify-center py-20">
      <p className="text-muted text-sm">대시보드 — 준비 중</p>
    </div>
  );
}

function AdminShell() {
  return (
    <SiteShell>
      <Outlet />
    </SiteShell>
  );
}

function ShellLoading() {
  return (
    <div className="py-20 px-4 space-y-4 max-w-screen-xl mx-auto">
      <Skeleton className="h-8 w-48" />
      <Skeleton className="h-24 w-full" />
      <Skeleton className="h-24 w-full" />
    </div>
  );
}

export function AdminApp() {
  return (
    <QueryClientProvider client={queryClient}>
      <BrowserRouter>
        <Routes>
          <Route
            path="/setup/*"
            element={
              <SetupGuard>
                <Suspense fallback={
                  <div className="min-h-screen flex items-center justify-center">
                    <div className="animate-pulse text-subtle">Loading...</div>
                  </div>
                }>
                  <SetupWizard />
                </Suspense>
              </SetupGuard>
            }
          />
          <Route element={<AdminShell />}>
            <Route
              index
              element={
                <SetupGuard fullPage={false}>
                  <HomeRedirect />
                </SetupGuard>
              }
            />
            <Route
              path="sites"
              element={
                <SetupGuard fullPage={false}>
                  <Suspense fallback={<ShellLoading />}>
                    <SitesPage />
                  </Suspense>
                </SetupGuard>
              }
            />
            <Route path="sites/new" element={<NewSiteWizardPage />} />
            <Route path="s/:slug" element={<DashboardPage />} />
          </Route>
        </Routes>
      </BrowserRouter>
    </QueryClientProvider>
  );
}
