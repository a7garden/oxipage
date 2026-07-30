import { useEffect, useState } from "react";
import { useParams } from "react-router";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { getConfig, updateConfig, type ConfigResponse } from "../shared/api";
import { Button } from "../../shared/ui/button";
import { Input } from "../../shared/ui/input";
import { Label } from "../../shared/ui/label";
import { Skeleton } from "../../shared/ui/skeleton";
import { Trash2 } from "lucide-react";

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
  const qc = useQueryClient();

  const { data, isLoading, isError } = useQuery({
    queryKey: ["site", slug, "config"],
    queryFn: () => getConfig(slug!),
    enabled: !!slug,
  });

  const [siteName, setSiteName] = useState("");
  const [baseUrl, setBaseUrl] = useState("");
  const [defaultLang, setDefaultLang] = useState("ko");
  const [defaultMode, setDefaultMode] = useState("grid");

  useEffect(() => {
    if (!data) return;
    setSiteName(data.site.name);
    setBaseUrl(data.site.base_url);
    setDefaultLang(data.site.default_lang);
    setDefaultMode(data.lobby.default_mode);
  }, [data]);

  const save = useMutation({
    mutationFn: () =>
      updateConfig(slug!, {
        site: { name: siteName, base_url: baseUrl, default_lang: defaultLang },
        lobby: { default_mode: defaultMode },
      }),
    onSuccess: (updated) => {
      qc.setQueryData(["site", slug, "config"], updated);
      qc.invalidateQueries({ queryKey: ["sites"] });
    },
  });

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
            options={["ko", "en"]}
          />
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
          <div className="text-xs text-muted space-y-1">
            <div>github_username: {data.integrations.github_username ?? "—"}</div>
            <div>tmdb_api_key_env: {data.integrations.tmdb_api_key_env ?? "OXIPAGE_TMDB_KEY"}</div>
            <div>aladin_ttbkey_env: {data.integrations.aladin_ttbkey_env ?? "OXIPAGE_ALADIN_TTBKEY"}</div>
          </div>
        </div>

        <div className="border border-[#fecaca] rounded-lg p-5">
          <h3 className="text-sm font-semibold mb-4 text-[#dc2626]">Danger Zone</h3>
          <div className="flex gap-2">
            <Button variant="outline" size="sm" className="border-red-300 text-red-600 hover:bg-red-50" disabled>
              <Trash2 size={14} className="mr-1" /> Purge All Data
            </Button>
            <Button variant="outline" size="sm" className="border-red-300 text-red-600 hover:bg-red-50" disabled>
              <Trash2 size={14} className="mr-1" /> Delete Site
            </Button>
          </div>
          <p className="text-xs text-muted mt-2">Use the Sites page to manage site deletion.</p>
        </div>
      </div>

      <div className="flex justify-end gap-2 mt-4">
        <Button
          variant="outline"
          onClick={() => {
            setSiteName(data.site.name);
            setBaseUrl(data.site.base_url);
            setDefaultLang(data.site.default_lang);
            setDefaultMode(data.lobby.default_mode);
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
