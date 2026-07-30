# Console Global UX — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Polish shell: CSS tokens, scroll reset, offline banner, markdown preview, SiteSelector info panel.

**Architecture:** Frontend-only — no new server endpoints (SiteSelector reuses S1's getStats). build_log.finished_at absorbed by S3. Responsive deferred.

**Tech Stack:** TypeScript/React (marked, TanStack Query, Tailwind v4)

## Global Constraints

- New tokens go in `:root` (sidebar stays dark always — never toggled by theme)
- No new server endpoints
- `ScrollRestoration` NOT available (BrowserRouter); use pathname scrollTo hook
- marked output is trusted (own content, local console) — no sanitization needed
- Reuse existing UI primitives (no restructuring)

---

### Task 1: Console CSS tokens

**Files:**
- Modify: `web/src/shared/tokens.css` (+--console-sidebar-* in :root)
- Modify: `web/src/admin/shell/Sidebar.tsx` (hardcoded hex → var())
- Modify: `web/src/admin/shell/Topbar.tsx` (logo hex → token)

- [ ] **Add tokens** in `:root` block (theme-agnostic): `--console-sidebar-bg: #1a1e24; --console-sidebar-text: #9ca3af; --console-sidebar-text-active: #4ade80; --console-sidebar-border-active: #22c55e; --console-sidebar-hover-bg: rgba(255,255,255,0.04); --console-sidebar-label: #6b7280;`
- [ ] **Sidebar.tsx**: replace all inline `#1a1e24` → `var(--console-sidebar-bg)`, `#6b7280` → `var(--console-sidebar-label)`, `#4ade80` → `var(--console-sidebar-text-active)`, `#22c55e` → `var(--console-sidebar-border-active)`, `#9ca3af` → `var(--console-sidebar-text)`, `rgba(34, 197, 94, 0.1)` → `color-mix(in srgb, var(--console-sidebar-border-active) 10%, transparent)`
- [ ] **Topbar.tsx**: logo `#2a6b4a` → `var(--p-accent-600)`

- [ ] `cd web && npx tsc --noEmit`

---

### Task 2: Scroll reset

**Files:**
- Create: `web/src/admin/shared/ui/ScrollToTop.tsx`
- Modify: `web/src/admin/App.tsx` (or ConsoleShell.tsx — mount component)

- [ ] **Create ScrollToTop**

```tsx
import { useEffect } from "react";
import { useLocation } from "react-router";

export function ScrollToTop() {
  const { pathname } = useLocation();
  useEffect(() => { window.scrollTo(0, 0); }, [pathname]);
  return null;
}
```
- [ ] **Mount** in `ConsoleShell` or `<Routes>` group: `<ScrollToTop />` before `<Outlet />` or <Routes>.

- [ ] `cd web && npx tsc --noEmit`

---

### Task 3: Offline indicator

**Files:**
- Create: `web/src/admin/shared/ui/OfflineBanner.tsx`
- Modify: `web/src/admin/shell/ConsoleShell.tsx` (mount after Topbar)

- [ ] **Create OfflineBanner**

```tsx
import { useState, useEffect } from "react";

export function OfflineBanner() {
  const [online, setOnline] = useState(() => navigator.onLine);
  useEffect(() => {
    const go = () => setOnline(true);
    const goff = () => setOnline(false);
    window.addEventListener("online", go);
    window.addEventListener("offline", goff);
    return () => { window.removeEventListener("online", go); window.removeEventListener("offline", goff); };
  }, []);
  if (online) return null;
  return (
    <div className="bg-yellow-100 border-b border-yellow-300 px-4 py-1.5 text-xs text-yellow-800 text-center">
      Offline — changes will retry when reconnected
    </div>
  );
}
```
- [ ] Mount in ConsoleShell: `<Topbar /><OfflineBanner /><div className="flex flex-1"><Sidebar /><main>...`

- [ ] `cd web && npx tsc --noEmit`

---

### Task 4: Markdown preview

**Files:**
- Modify: `web/package.json` (add `marked`)
- Create: `web/src/admin/shared/ui/MarkdownEditor.tsx`
- Modify: `web/src/admin/content/*Tab.tsx` (replace body Textarea in blog, novels, scraps, movies)

- [ ] **Add `marked` dep**: `cd web && bun add marked`
- [ ] **Create MarkdownEditor**

```tsx
import { useState } from "react";
import { marked } from "marked";
import { Textarea } from "../../../shared/ui/textarea";

interface Props {
  value: string;
  onChange: (v: string) => void;
  rows?: number;
  placeholder?: string;
}

export function MarkdownEditor({ value, onChange, rows = 6, placeholder }: Props) {
  const [mode, setMode] = useState<"edit" | "preview">("edit");
  return (
    <div className="border border-line rounded overflow-hidden">
      <div className="flex gap-0 border-b border-line bg-surface/30">
        <button onClick={() => setMode("edit")} className={`px-3 py-1 text-xs font-medium ${mode==="edit" ? "bg-canvas text-foreground" : "text-muted hover:text-foreground"}`}>Edit</button>
        <button onClick={() => setMode("preview")} className={`px-3 py-1 text-xs font-medium ${mode==="preview" ? "bg-canvas text-foreground" : "text-muted hover:text-foreground"}`}>Preview</button>
      </div>
      {mode === "edit" ? (
        <Textarea value={value} onChange={e => onChange(e.target.value)} rows={rows} placeholder={placeholder} className="border-0 rounded-none" />
      ) : (
        <div className="p-3 text-sm prose prose-sm max-w-none" dangerouslySetInnerHTML={{ __html: marked.parse(value || "") }} />
      )}
    </div>
  );
}
```
- [ ] **Replace body Textarea** in content tabs: import `MarkdownEditor`, replace `<Textarea>` for body fields.

- [ ] `cd web && npx tsc --noEmit`

---

### Task 5: SiteSelector info panel

**Files:**
- Modify: `web/src/admin/shell/SiteSelector.tsx`

- [ ] **Add stats query** for current site:

```tsx
const { data: stats } = useQuery({
  queryKey: ["site", current.name, "stats"],
  queryFn: () => getStats(current.name),
  enabled: open && !!current.name,
});
```

- [ ] **Render info panel** in the dropdown's right area (or as part of each site row):

```tsx
// Inside the dropdown, alongside or below the site list:
{stats && (
  <div className="mt-3 pt-3 border-t border-line text-xs text-muted space-y-1">
    <div className="flex justify-between"><span>Content</span><span>{Object.values(stats.data.counts).reduce((a: number, b: number) => a + b, 0)} entries</span></div>
    <div className="flex justify-between"><span>Storage</span><span>{formatBytes(stats.data.storage_bytes)}</span></div>
    <div className="flex justify-between"><span>Last build</span><span>{stats.data.last_build ? new Date(stats.data.last_build.started_at).toLocaleDateString() : "Never"}</span></div>
  </div>
)}
```

- [ ] `cd web && npx tsc --noEmit`

---

### Task 6: Full check + smoke

- [ ] `cargo check && cd web && npx tsc --noEmit`
- [ ] Manual: toggle dark/light → sidebar stays dark, main switches; navigate pages → scroll resets; disable network → banner; open blog editor → Edit/Preview toggle renders markdown; SiteSelector shows stats
