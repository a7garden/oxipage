import { BrowserRouter, Routes, Route } from "react-router";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { AdminShell } from "./shell/AdminShell";
import { DashboardPage } from "./dashboard/DashboardPage";
import { ExtensionsPage } from "./extensions/ExtensionsPage";
import { ThemesPage } from "./themes/ThemesPage";
import { BlogListPage } from "./content/BlogListPage";
import { BlogEditorPage } from "./content/BlogEditorPage";
import { DataBrowserPage } from "./content/DataBrowserPage";
import { SettingsPage } from "./settings/SettingsPage";

const queryClient = new QueryClient();

export function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <BrowserRouter>
        <Routes>
          <Route element={<AdminShell />}>
            <Route index element={<DashboardPage />} />
            <Route path="extensions" element={<ExtensionsPage />} />
            <Route path="content/blog" element={<BlogListPage />} />
            <Route path="content/blog/new" element={<BlogEditorPage />} />
            <Route path="content/blog/:slug" element={<BlogEditorPage />} />
            <Route path="content/:extId" element={<DataBrowserPage />} />
            <Route path="themes" element={<ThemesPage />} />
            <Route path="settings" element={<SettingsPage />} />
          </Route>
        </Routes>
      </BrowserRouter>
    </QueryClientProvider>
  );
}
