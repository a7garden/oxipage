import { useState } from "react";
import { useParams } from "react-router";
import { Button } from "../../shared/ui/button";

function SettingsField({
  label,
  defaultValue,
  type,
  options,
  disabled,
  placeholder,
}: {
  label: string;
  defaultValue?: string;
  type?: "text" | "password" | "select";
  options?: string[];
  disabled?: boolean;
  placeholder?: string;
}) {
  const [value, setValue] = useState(defaultValue ?? "");
  return (
    <div className="flex items-center gap-3 mb-2.5">
      <label className="text-xs text-muted w-24 shrink-0 text-right">{label}</label>
      {type === "select" ? (
        <select
          value={value}
          onChange={(e) => setValue(e.target.value)}
          className="flex-1 max-w-sm px-2.5 py-1.5 border border-line rounded-md text-sm bg-surface/50"
          disabled={disabled}
        >
          {options?.map((opt) => (
            <option key={opt} value={opt}>
              {opt}
            </option>
          ))}
        </select>
      ) : (
        <input
          type={type ?? "text"}
          value={value}
          onChange={(e) => setValue(e.target.value)}
          placeholder={placeholder}
          className="flex-1 max-w-sm px-2.5 py-1.5 border border-line rounded-md text-sm bg-surface/50"
          disabled={disabled}
        />
      )}
    </div>
  );
}

export function SettingsPage() {
  const { slug } = useParams<{ slug: string }>()!;

  return (
    <div>
      <h1 className="text-xl font-bold text-foreground mb-1">Settings</h1>
      <p className="text-sm text-muted mb-6">Site-wide configuration for {slug}</p>

      <div className="space-y-4">
        <div className="border border-line rounded-lg p-5">
          <h3 className="text-sm font-semibold mb-4">General</h3>
          <SettingsField label="Site Title" defaultValue="My Blog" />
          <SettingsField label="Base URL" defaultValue={`https://${slug}.example.com`} />
          <SettingsField label="Language" defaultValue="ko" type="select" options={["ko", "en", "ko, en"]} />
        </div>

        <div className="border border-line rounded-lg p-5">
          <h3 className="text-sm font-semibold mb-4">Display</h3>
          <SettingsField label="Default Mode" defaultValue="grid" type="select" options={["grid", "list", "canvas"]} />
          <SettingsField label="Profile" defaultValue="developer" type="select" options={["developer", "writer", "artist"]} />
        </div>

        <div className="border border-line rounded-lg p-5">
          <h3 className="text-sm font-semibold mb-4">API Tokens</h3>
          <SettingsField label="TMDB Key" defaultValue="" type="password" placeholder="••••••••••••••••" />
          <SettingsField label="Aladin Key" defaultValue="" type="password" disabled placeholder="not set" />
          <SettingsField label="GitHub User" defaultValue="oxi" />
        </div>

        <div className="border border-[#fecaca] rounded-lg p-5">
          <h3 className="text-sm font-semibold mb-4 text-[#dc2626]">Danger Zone</h3>
          <div className="flex gap-2">
            <Button variant="outline" size="sm" className="border-red-300 text-red-600 hover:bg-red-50">
              Purge All Data
            </Button>
            <Button variant="outline" size="sm" className="border-red-300 text-red-600 hover:bg-red-50">
              Delete Site
            </Button>
          </div>
        </div>
      </div>

      <div className="flex justify-end gap-2 mt-4">
        <Button variant="outline">Reset</Button>
        <Button>Save Changes</Button>
      </div>
    </div>
  );
}
