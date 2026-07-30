import { lazy, Suspense } from "react";
import { BrowserRouter, Routes, Route } from "react-router";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { ConsoleShell } from "./shell/ConsoleShell";
import { ScrollToTop } from "./shared/ui/ScrollToTop";
import { HomeRedirect } from "./sites/HomeRedirect";
import { SitesPage } from "./sites/SitesPage";
import { NewSiteWizardPage } from "./sites/NewSiteWizardPage";
import { SetupGuard } from "../setup/SetupGuard";
import { Skeleton } from "../shared/ui/skeleton";

const DashboardPage = lazy(() =>
  import("./dashboard/DashboardPage").then((m) => ({ default: m.DashboardPage })),
);

const ContentPage = lazy(() =>
  import("./content/ContentPage").then((m) => ({ default: m.ContentPage })),
);

const ExtensionsPage = lazy(() =>
  import("./extensions/ExtensionsPage").then((m) => ({ default: m.ExtensionsPage })),
);

const ThemesPage = lazy(() =>
  import("./themes/ThemesPage").then((m) => ({ default: m.ThemesPage })),
);

const DeployPage = lazy(() =>
  import("./deploy/DeployPage").then((m) => ({ default: m.DeployPage })),
);

const SettingsPage = lazy(() =>
  import("./settings/SettingsPage").then((m) => ({ default: m.SettingsPage })),
);

const SetupWizard = lazy(() =>
  import("../setup/SetupWizard").then((m) => ({ default: m.SetupWizard })),
);

const queryClient = new QueryClient();

function ShellFallback() {
  return (
    <div className="flex">
      <aside className="w-[200px]" style={{ backgroundColor: "#1a1e24", minHeight: "100vh" }} />
      <div className="flex-1 p-6 space-y-4">
        <Skeleton className="h-8 w-48" />
        <Skeleton className="h-32 w-full" />
        <Skeleton className="h-32 w-full" />
      </div>
    </div>
  );
}

export function AdminApp() {
  return (
    <QueryClientProvider client={queryClient}>
      <BrowserRouter>
        <ScrollToTop />
        <Routes>
          <Route
            path="/setup/*"
            element={
              <SetupGuard>
                <Suspense
                  fallback={
                    <div className="min-h-screen flex items-center justify-center">
                      <div className="animate-pulse text-subtle">Loading...</div>
                    </div>
                  }
                >
                  <SetupWizard />
                </Suspense>
              </SetupGuard>
            }
          />
          <Route element={<ConsoleShell />}>
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
                  <SitesPage />
                </SetupGuard>
              }
            />
            <Route path="sites/new" element={<NewSiteWizardPage />} />
            <Route
              path="s/:slug"
              element={
                <Suspense fallback={<ShellFallback />}>
                  <DashboardPage />
                </Suspense>
              }
            />
            <Route
              path="s/:slug/content"
              element={
                <Suspense fallback={<ShellFallback />}>
                  <ContentPage />
                </Suspense>
              }
            />
            <Route
              path="s/:slug/extensions"
              element={
                <Suspense fallback={<ShellFallback />}>
                  <ExtensionsPage />
                </Suspense>
              }
            />
            <Route
              path="s/:slug/themes"
              element={
                <Suspense fallback={<ShellFallback />}>
                  <ThemesPage />
                </Suspense>
              }
            />
            <Route
              path="s/:slug/deploy"
              element={
                <Suspense fallback={<ShellFallback />}>
                  <DeployPage />
                </Suspense>
              }
            />
            <Route
              path="s/:slug/settings"
              element={
                <Suspense fallback={<ShellFallback />}>
                  <SettingsPage />
                </Suspense>
              }
            />
          </Route>
        </Routes>
      </BrowserRouter>
    </QueryClientProvider>
  );
}
