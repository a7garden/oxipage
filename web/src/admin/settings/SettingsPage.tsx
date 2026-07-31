import { useEffect, useState } from "react";
import { useParams, useNavigate, Link } from "react-router";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { getConfig, updateConfig, removeSite, listSites, getDefaultSite, setDefaultSite, getTheme, type ConfigResponse } from "../shared/api";
import { Button } from "../../shared/ui/button";
import { Input } from "../../shared/ui/input";
import { Skeleton } from "../../shared/ui/skeleton";
import { Trash2, X } from "lucide-react";
import { ThemeToggle } from "../../shared/ThemeToggle";

function SettingsField({
  label,
  value,
  onChange,
  type,
  options,
  placeholder,
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
  type?: "text" | "password" | "select";
  options?: string[];
  placeholder?: string;
}) {
  return (
    <div className="flex items-center gap-3 mb-2.5">
      <label className="text-xs text-muted w-24 shrink-0 text-right">{label}</label>
      {type === "select" ? (
        <select
          value={value}
          onChange={(e) => onChange(e.target.value)}
          className="flex-1 px-3 py-1.5 border border-line rounded-md text-sm bg-surface/50"
        >
          {(options ?? []).map((o) => (
            <option key={o} value={o}>{o}</option>
          ))}
        </select>
      ) : (
        <Input
          type={type ?? "text"}
          value={value}
          onChange={(e) => onChange(e.target.value)}
          placeholder={placeholder}
          className="flex-1"
        />
      )}
    </div>
  );
}

export function SettingsPage() {
  const { slug } = useParams<{ slug: string }>()!;
  const navigate = useNavigate();
  const qc = useQueryClient();

  const { data, isLoading, isError } = useQuery({
    queryKey: ["site", slug, "config"],
    queryFn: () => getConfig(slug!),
    enabled: !!slug,
  });

  // General
  const [siteName, setSiteName] = useState("");
  const [baseUrl, setBaseUrl] = useState("");
  const [defaultLang, setDefaultLang] = useState("ko");
  const [languages, setLanguages] = useState<string[]>(["ko"]);
  const [defaultMode, setDefaultMode] = useState("grid");

  // Integrations
  const [githubUsername, setGithubUsername] = useState("");
  const [tmdbApiKeyEnv, setTmdbApiKeyEnv] = useState("");
  const [aladinTtbkeyEnv, setAladinTtbkeyEnv] = useState("");

  // Deployment · GitHub Pages
  const [owner, setOwner] = useState("");
  const [repo, setRepo] = useState("");
  const [branch, setBranch] = useState("gh-pages");

  // Danger Zone
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [deleteInput, setDeleteInput] = useState("");

  // Language chip editor
  const [newLang, setNewLang] = useState("");
  // Default site (global) selector
  const { data: sitesData } = useQuery({ queryKey: ["sites"], queryFn: listSites });
  const { data: defaultData } = useQuery({ queryKey: ["default-site"], queryFn: getDefaultSite });
  const sites = sitesData?.data ?? [];
  const currentDefault = defaultData?.data.default_site ?? null;
  const [defaultSiteSel, setDefaultSiteSel] = useState("");
  const { data: themeData } = useQuery({
    queryKey: ["site", slug, "theme"],
    queryFn: () => getTheme(slug!),
    enabled: !!slug,
  });
  const setDefault = useMutation({
    mutationFn: setDefaultSite,
    onSuccess: () => qc.invalidateQueries({ queryKey: ["default-site"] }),
  });
  useEffect(() => {
    if (currentDefault) setDefaultSiteSel(currentDefault);
  }, [currentDefault]);

  useEffect(() => {
    if (!data) return;
    setSiteName(data.site.name);
    setBaseUrl(data.site.base_url);
    setDefaultLang(data.site.default_lang);
    setLanguages(data.site.languages.length > 0 ? data.site.languages : ["ko"]);
    setDefaultMode(data.lobby.default_mode);
    setGithubUsername(data.integrations.github_username ?? "");
    setTmdbApiKeyEnv(data.integrations.tmdb_api_key_env ?? "");
    setAladinTtbkeyEnv(data.integrations.aladin_ttbkey_env ?? "");
    const pages = data.deploy?.github_pages;
    setOwner(pages?.owner ?? "");
    setRepo(pages?.repo ?? "");
    setBranch(pages?.branch ?? "gh-pages");
  }, [data]);

  const save = useMutation({
    mutationFn: () =>
      updateConfig(slug!, {
        site: {
          name: siteName,
          base_url: baseUrl,
          default_lang: defaultLang,
          languages,
        },
        lobby: { default_mode: defaultMode },
        integrations: {
          github_username: githubUsername || null,
          tmdb_api_key_env: tmdbApiKeyEnv || null,
          aladin_ttbkey_env: aladinTtbkeyEnv || null,
        },
        deploy: {
          github_pages: validTarget ? { owner, repo, branch } : null,
        },
      }),
    onSuccess: (updated) => {
      qc.setQueryData(["site", slug, "config"], updated);
      qc.invalidateQueries({ queryKey: ["sites"] });
    },
  });

  const componentRe = /^[A-Za-z0-9._-]+$/;
  const branchOk =
    /^[A-Za-z0-9._\/-]+$/.test(branch) &&
    !branch.includes("..") &&
    !branch.startsWith("/") &&
    !branch.endsWith("/");
  const validTarget =
    componentRe.test(owner) && componentRe.test(repo) && branchOk && owner !== "" && repo !== "";
  const pagesUrl = validTarget
    ? repo.toLowerCase() === `${owner.toLowerCase()}.github.io`
      ? `https://${owner}.github.io/`
      : `https://${owner}.github.io/${repo}/`
    : "";
  const basePath = validTarget
    ? repo.toLowerCase() === `${owner.toLowerCase()}.github.io`
      ? "/"
      : `/${repo}/`
    : "";

  const handleDeleteSite = async () => {
    try {
      await removeSite(slug!);
      navigate("/sites");
    } catch (e) {
      alert(e instanceof Error ? e.message : "Delete failed");
    }
  };

  const addLanguage = () => {
    const code = newLang.trim().toLowerCase();
    if (code && !languages.includes(code)) {
      setLanguages([...languages, code]);
    }
    setNewLang("");
  };

  const removeLanguage = (code: string) => {
    setLanguages(languages.filter((l) => l !== code));
  };

  if (isLoading) {
    return (
      <div className="space-y-4">
        <Skeleton className="h-8 w-48" />
        <Skeleton className="h-48 w-full" />
        <Skeleton className="h-32 w-full" />
      </div>
    );
  }

  if (isError || !data) {
    return (
      <div className="border border-line rounded-lg p-6 text-center text-muted text-sm">
        Failed to load configuration.{" "}
        <button onClick={() => qc.invalidateQueries({ queryKey: ["site", slug, "config"] })} className="underline">Retry</button>
      </div>
    );
  }

  return (
    <div>
      <h1 className="text-xl font-bold text-foreground mb-1">Settings</h1>
      <p className="text-sm text-muted mb-6">Site-wide configuration for {slug}</p>

      <div className="space-y-4">
        <div className="border border-line rounded-lg p-5">
          <h3 className="text-sm font-semibold mb-4">General</h3>
          <SettingsField label="Site Title" value={siteName} onChange={setSiteName} />
          <SettingsField label="Base URL" value={baseUrl} onChange={setBaseUrl} />
          <SettingsField
            label="Default Lang"
            value={defaultLang}
            onChange={setDefaultLang}
            type="select"
            options={languages}
          />
          <div className="flex items-start gap-3 mb-2.5">
            <label className="text-xs text-muted w-24 shrink-0 text-right pt-1.5">Languages</label>
            <div className="flex-1">
              <div className="flex flex-wrap gap-1.5 mb-1.5">
                {languages.map((code) => (
                  <span
                    key={code}
                    className="inline-flex items-center gap-1 px-2 py-0.5 text-xs rounded bg-surface/50 border border-line"
                  >
                    {code}
                    <button
                      onClick={() => removeLanguage(code)}
                      className="text-muted hover:text-red-600"
                      aria-label={`Remove ${code}`}
                    >
                      <X size={12} />
                    </button>
                  </span>
                ))}
              </div>
              <div className="flex gap-1.5">
                <Input
                  value={newLang}
                  onChange={(e) => setNewLang(e.target.value)}
                  placeholder="e.g. ja"
                  className="w-24 text-xs"
                  onKeyDown={(e) => e.key === "Enter" && (e.preventDefault(), addLanguage())}
                />
                <Button variant="outline" size="sm" onClick={addLanguage} disabled={!newLang.trim()}>
                  Add
                </Button>
              </div>
            </div>
          </div>
        </div>

        <div className="border border-line rounded-lg p-5">
          <h3 className="text-sm font-semibold mb-4">Display</h3>
          <SettingsField
            label="Default Mode"
            value={defaultMode}
            onChange={setDefaultMode}
            type="select"
            options={["grid", "list", "canvas"]}
          />
        </div>

        <div className="border border-line rounded-lg p-5">
          <h3 className="text-sm font-semibold mb-4">Integrations</h3>
          <p className="text-xs text-muted mb-2">
            Set environment-variable names in <code className="px-1 py-0.5 bg-surface/50 rounded">oxipage.toml</code>{" "}
            <code className="px-1 py-0.5 bg-surface/50 rounded">[integrations]</code>. The console never stores
            secret values.
          </p>
          <SettingsField label="GitHub User" value={githubUsername} onChange={setGithubUsername} placeholder="a7garden" />
          <SettingsField
            label="TMDB Key Env"
            value={tmdbApiKeyEnv}
            onChange={setTmdbApiKeyEnv}
            placeholder="OXIPAGE_TMDB_KEY"
          />
          <SettingsField
            label="Aladin Key Env"
            value={aladinTtbkeyEnv}
            onChange={setAladinTtbkeyEnv}
            placeholder="OXIPAGE_ALADIN_TTBKEY"
          />
        </div>

        <div className="border border-line rounded-lg p-5">
          <h3 className="text-sm font-semibold mb-1">Deployment · GitHub Pages</h3>
          <p className="text-xs text-muted mb-3">
            Authentication stays in GitHub CLI; no token is stored.
          </p>
          <SettingsField label="Owner" value={owner} onChange={setOwner} placeholder="github-username" />
          <SettingsField label="Repository" value={repo} onChange={setRepo} placeholder="my-site" />
          <SettingsField label="Publish branch" value={branch} onChange={setBranch} placeholder="gh-pages" />
          <div className="ml-28 text-xs text-muted mb-3">
            Pages URL: {pagesUrl || "—"}
            <br />
            Base path: <code className="font-mono">{basePath || "—"}</code>
          </div>
          <div className="flex gap-2">
            <Button
              type="button"
              variant="outline"
              disabled={!pagesUrl || baseUrl === pagesUrl}
              onClick={() => setBaseUrl(pagesUrl)}
            >
              Use this Pages URL as Site Base URL
            </Button>
            <Button type="button" variant="outline" asChild>
              <Link to={`/sites/${slug}/deploy`}>Open Deploy page</Link>
            </Button>
          </div>
        </div>

        <div className="border border-line rounded-lg p-5">
          <h3 className="text-sm font-semibold mb-1">Default Site</h3>
          <p className="text-xs text-muted mb-3">
            The site opened by default when the console starts.
          </p>
          <div className="flex items-center gap-2">
            <select
              value={defaultSiteSel || currentDefault || ""}
              onChange={(e) => setDefaultSiteSel(e.target.value)}
              className="border border-line rounded px-2 py-1 text-sm bg-canvas max-w-xs"
            >
              {sites.map((s) => (
                <option key={s.name} value={s.name}>{s.name}</option>
              ))}
            </select>
            <Button
              variant="outline"
              size="sm"
              disabled={setDefault.isPending || !defaultSiteSel || defaultSiteSel === currentDefault}
              onClick={() => setDefault.mutate(defaultSiteSel)}
            >
              {setDefault.isPending ? "Setting…" : "Set default"}
            </Button>
            {setDefault.isSuccess && (
              <span className="text-xs text-[#16a34a]">Updated</span>
            )}
          </div>
        </div>

        <div className="border border-line rounded-lg p-5">
          <h3 className="text-sm font-semibold mb-4">Appearance</h3>
          <div className="flex items-center gap-3 mb-3">
            <div className="text-xs text-muted w-32">Console appearance</div>
            <ThemeToggle />
          </div>
          <div className="flex items-center gap-3">
            <div className="text-xs text-muted w-32">Public site theme</div>
            <div className="text-sm font-medium" data-testid="public-theme-name">
              {themeData?.definition?.name_en ?? "—"}
            </div>
            <Link
              to={`/s/${slug}/themes`}
              className="ml-auto text-xs text-primary hover:underline"
            >
              Open full theme editor →
            </Link>
          </div>
        </div>

        <div className="border border-[#fecaca] rounded-lg p-5">
          <h3 className="text-sm font-semibold mb-4 text-[#dc2626]">Danger Zone</h3>
          {!confirmDelete ? (
            <div>
              <Button
                variant="outline"
                size="sm"
                className="border-red-300 text-red-600 hover:bg-red-50"
                onClick={() => setConfirmDelete(true)}
              >
                <Trash2 size={14} className="mr-1" /> Delete Site
              </Button>
              <Button
                variant="outline"
                size="sm"
                disabled
                className="border-red-300 text-red-400 ml-2"
              >
                Purge All Data (Coming soon)
              </Button>
              <p className="text-xs text-muted mt-2">
                Registry-only removal — site files on disk are preserved.
              </p>
            </div>
          ) : (
            <div className="border border-red-300 rounded-md p-4 bg-red-50/30">
              <p className="text-sm font-medium text-red-700 mb-2">
                Are you sure you want to delete <strong>{slug}</strong>?
              </p>
              <p className="text-xs text-red-600 mb-3">
                Type <code className="px-1 py-0.5 bg-red-100 rounded">{slug}</code> below to confirm.
              </p>
              <Input
                value={deleteInput}
                onChange={(e) => setDeleteInput(e.target.value)}
                placeholder={slug}
                className="mb-3 text-sm"
              />
              <div className="flex gap-2">
                <Button
                  size="sm"
                  className="bg-red-600 text-white hover:bg-red-700"
                  disabled={deleteInput !== slug}
                  onClick={handleDeleteSite}
                >
                  <Trash2 size={14} className="mr-1" /> Confirm Delete
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => {
                    setConfirmDelete(false);
                    setDeleteInput("");
                  }}
                >
                  Cancel
                </Button>
              </div>
            </div>
          )}
        </div>
      </div>

      <div className="flex justify-end gap-2 mt-4">
        <Button
          variant="outline"
          onClick={() => {
            setSiteName(data.site.name);
            setBaseUrl(data.site.base_url);
            setDefaultLang(data.site.default_lang);
            setLanguages(data.site.languages.length > 0 ? data.site.languages : ["ko"]);
            setDefaultMode(data.lobby.default_mode);
            setGithubUsername(data.integrations.github_username ?? "");
            setTmdbApiKeyEnv(data.integrations.tmdb_api_key_env ?? "");
            setAladinTtbkeyEnv(data.integrations.aladin_ttbkey_env ?? "");
            setConfirmDelete(false);
            setDeleteInput("");
          }}
          disabled={save.isPending}
        >
          Reset
        </Button>
        <Button onClick={() => save.mutate()} disabled={save.isPending}>
          {save.isPending ? "Saving..." : save.isSuccess ? "Saved!" : "Save Changes"}
        </Button>
      </div>
      {save.isError && (
        <p className="text-sm text-red-600 text-right mt-2">
          {save.error instanceof Error ? save.error.message : "Save failed"}
        </p>
      )}
    </div>
  );
}
