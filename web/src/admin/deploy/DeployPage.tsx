import { useParams } from "react-router";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";
import {
  listBuilds,
  listDeploys,
  triggerBuild,
  triggerDeploy,
  getDeployPreflight,
  getCurrentOperation,
  operationStreamUrl,
  previewSiteUrl,
  OperationConflictError,
  type BuildRecord,
  type DeployRecord,
} from "../shared/api";
import { Button } from "../../shared/ui/button";
import { Badge } from "../../shared/ui/badge";
import { Skeleton } from "../../shared/ui/skeleton";

type OpKind = "build" | "deploy";
type RunStatus = "idle" | "running" | "done" | "failed";

interface LogLine {
  text: string;
  tone: "info" | "ok" | "err";
}

interface OpEvent {
  event: string;
  total?: number;
  ext_id?: string;
  pages?: number;
  total_pages?: number;
  count?: number;
  url?: string;
  commit?: string;
  branch?: string;
  error?: string;
  [k: string]: unknown;
}

const TERMINAL_EVENTS: Record<string, true> = {
  build_complete: true,
  build_failed: true,
  deployed: true,
  unchanged: true,
  failed: true,
};

function formatEvent(ev: OpEvent): LogLine {
  switch (ev.event) {
    case "build_started":
      return { text: `▶ Build started (${ev.total} extensions)`, tone: "info" };
    case "extension_start":
      return { text: `  ${ev.ext_id} …`, tone: "info" };
    case "extension_done":
      return { text: `  ✓ ${ev.ext_id} → ${ev.pages} pages`, tone: "ok" };
    case "build_complete":
      return { text: `✓ Build complete — ${ev.total_pages} pages`, tone: "ok" };
    case "build_failed":
      return { text: `✗ Build failed: ${ev.error}`, tone: "err" };
    case "preflight_started":
      return { text: "▶ Preflight checks…", tone: "info" };
    case "gh_ready":
      return { text: "✓ GitHub CLI ready", tone: "ok" };
    case "auth_ready":
      return { text: "✓ Authenticated", tone: "ok" };
    case "repository_ready":
      return { text: "✓ Repository verified", tone: "ok" };
    case "worktree_ready":
      return { text: "✓ Prepared gh-pages worktree", tone: "ok" };
    case "files_copied":
      return { text: `✓ Copied ${ev.count} files`, tone: "ok" };
    case "commit_created":
      return { text: `✓ Committed ${String(ev.commit).slice(0, 8)}`, tone: "ok" };
    case "pushing":
      return { text: `▶ Pushing to ${ev.branch}…`, tone: "info" };
    case "deployed":
      return { text: `✓ Deployed: ${ev.url}`, tone: "ok" };
    case "unchanged":
      return { text: `• Unchanged (no diff): ${ev.url}`, tone: "info" };
    case "failed":
      return { text: `✗ Deploy failed: ${ev.error}`, tone: "err" };
    default:
      return { text: JSON.stringify(ev), tone: "info" };
  }
}

const PREVIEW_DISABLED_CODES = new Set(["build_required", "missing_index", "stale_build_base", "stale_build_theme"]);

export function DeployPage() {
  const { slug } = useParams<{ slug: string }>()!;
  const qc = useQueryClient();
  const [lines, setLines] = useState<LogLine[]>([]);
  const [status, setStatus] = useState<RunStatus>("idle");
  const [op, setOp] = useState<OpKind | null>(null);
  const esRef = useRef<EventSource | null>(null);

  const buildsQ = useQuery({
    queryKey: ["site", slug, "builds"],
    queryFn: () => listBuilds(slug!),
    enabled: !!slug,
  });
  const deploysQ = useQuery({
    queryKey: ["site", slug, "deploys"],
    queryFn: () => listDeploys(slug!),
    enabled: !!slug,
  });
  const preflightQ = useQuery({
    queryKey: ["site", slug, "deploy-preflight"],
    queryFn: () => getDeployPreflight(slug!),
    enabled: !!slug,
    refetchInterval: 15000,
  });
  const currentQ = useQuery({
    queryKey: ["site", slug, "operation"],
    queryFn: () => getCurrentOperation(slug!),
    enabled: !!slug,
  });

  // Reattach to an in-flight operation on mount / when one appears.
  useEffect(() => {
    const cur = currentQ.data;
    if (cur?.active && !esRef.current) {
      attachStream(cur.kind, cur.run_id);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [currentQ.data?.run_id, currentQ.data?.active]);

  // Close any open SSE stream on unmount.
  useEffect(() => () => esRef.current?.close(), []);

  const builds: BuildRecord[] = buildsQ.data?.data ?? [];
  const deploys: DeployRecord[] = deploysQ.data ?? [];
  const last = builds[0];
  const busy = status === "running";
  const stale =
    preflightQ.data?.problems.some(
      (p) => p.code === "stale_build_base" || p.code === "stale_build_theme",
    ) ?? false;
  const hasBuild = !preflightQ.data?.problems.some(
    (p) => p.code === "build_required" || p.code === "missing_index",
  );

  function invalidateAll() {
    qc.invalidateQueries({ queryKey: ["site", slug, "builds"] });
    qc.invalidateQueries({ queryKey: ["site", slug, "deploys"] });
    qc.invalidateQueries({ queryKey: ["site", slug, "deploy-preflight"] });
    qc.invalidateQueries({ queryKey: ["site", slug, "operation"] });
    qc.invalidateQueries({ queryKey: ["site", slug, "stats"] });
  }

  function attachStream(kind: OpKind, id: string) {
    esRef.current?.close();
    setStatus("running");
    setOp(kind);
    const es = new EventSource(operationStreamUrl(slug!, kind, id));
    esRef.current = es;
    es.onmessage = (e) => {
      try {
        const ev = JSON.parse(e.data) as OpEvent;
        setLines((prev) => [...prev, formatEvent(ev)]);
        if (TERMINAL_EVENTS[ev.event]) {
          const failed =
            ev.event === "build_failed" || ev.event === "failed";
          setStatus(failed ? "failed" : "done");
          es.close();
          esRef.current = null;
          invalidateAll();
        }
      } catch {
        /* ignore malformed event */
      }
    };
    es.onerror = () => {
      // Stream closed (run finished elsewhere, or no active run).
      es.close();
      esRef.current = null;
      setStatus((s) => (s === "running" ? "idle" : s));
    };
  }

  async function onBuild() {
    setLines([]);
    setStatus("running");
    setOp("build");
    try {
      const res = await triggerBuild(slug!);
      attachStream("build", res.data.build_id);
    } catch (e) {
      if (e instanceof OperationConflictError && e.kind === "build") {
        setLines([{ text: "A build is already running — attaching…", tone: "info" }]);
        attachStream("build", e.id);
      } else {
        setStatus("failed");
        setLines([{ text: e instanceof Error ? e.message : "Build failed", tone: "err" }]);
      }
    }
  }

  async function onDeploy() {
    setLines([]);
    setStatus("running");
    setOp("deploy");
    try {
      const res = await triggerDeploy(slug!);
      attachStream("deploy", res.data.deploy_id);
    } catch (e) {
      if (e instanceof OperationConflictError && e.kind === "deploy") {
        setLines([{ text: "A deploy is already running — attaching…", tone: "info" }]);
        attachStream("deploy", e.id);
      } else {
        setStatus("failed");
        setLines([{ text: e instanceof Error ? e.message : "Deploy failed", tone: "err" }]);
      }
    }
  }

  return (
    <div>
      <div className="flex items-start justify-between mb-6">
        <div>
          <h1 className="text-xl font-bold text-foreground">Deploy</h1>
          <p className="text-sm text-muted mt-0.5">
            Build static site and deploy to GitHub Pages
          </p>
        </div>
        <div className="flex gap-2">
          <Button variant="outline" onClick={onBuild} disabled={busy}>
            {busy && op === "build" ? "Building…" : "↧ Build"}
          </Button>
          <Button
            variant="outline"
            asChild
            disabled={!hasBuild || stale}
          >
            {hasBuild && !stale ? (
              <a href={previewSiteUrl(slug!)} target="_blank" rel="noreferrer">
                Preview Site ↗
              </a>
            ) : (
              <span>Preview Site ↗</span>
            )}
          </Button>
          {stale && <Badge variant="warning">Stale build</Badge>}
          <Button onClick={onDeploy} disabled={busy || !preflightQ.data?.build_compatible}>
            {busy && op === "deploy" ? "Deploying…" : "⇧ Deploy"}
          </Button>
        </div>
      </div>

      {lines.length > 0 && (
        <div className="mb-6">
          <div className="flex items-center justify-between mb-2">
            <h2 className="text-sm font-semibold text-foreground">
              {op === "build" ? "Build" : "Deploy"} log
              {busy && (
                <span className="ml-2 text-muted font-normal">running…</span>
              )}
            </h2>
          </div>
          <div className="border border-line rounded-lg bg-[#0d1117] p-4 font-mono text-xs leading-relaxed max-h-80 overflow-auto">
            {lines.map((l, i) => (
              <div
                key={i}
                className={
                  l.tone === "ok"
                    ? "text-[#3fb950]"
                    : l.tone === "err"
                      ? "text-[#f85149]"
                      : "text-[#c9d1d9]"
                }
              >
                {l.text}
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Preflight card */}
      <section className="rounded-lg border border-line p-5 mb-6">
        <div className="flex items-center justify-between mb-3">
          <h2 className="text-sm font-semibold text-foreground">
            Deployment preflight
          </h2>
          {preflightQ.data?.build_compatible && (
            <Badge variant="positive">Ready</Badge>
          )}
        </div>
        {!preflightQ.data ? (
          <Skeleton className="h-16 w-full" />
        ) : (
          <div className="space-y-2">
            {preflightQ.data.problems.map((p) => (
              <div key={p.code} className="flex justify-between items-center text-sm">
                <span className="text-muted">{p.message}</span>
                {(p.action === "build" || p.action === "rebuild") && (
                  <Button size="sm" variant="outline" onClick={onBuild}>
                    Build
                  </Button>
                )}
                {p.action === "open_settings" && (
                  <Button size="sm" variant="outline" asChild>
                    <a href={`/sites/${slug}/settings`}>Settings</a>
                  </Button>
                )}
              </div>
            ))}
            {preflightQ.data.problems.length === 0 && (
              <p className="text-sm text-muted">All checks pass.</p>
            )}
            {preflightQ.data.pages_url && (
              <a
                href={preflightQ.data.pages_url}
                target="_blank"
                rel="noreferrer"
                className="text-xs text-muted underline"
              >
                {preflightQ.data.pages_url}
              </a>
            )}
          </div>
        )}
      </section>

      <h2 className="text-sm font-semibold text-foreground mb-3">Last Build</h2>
      {buildsQ.isLoading ? (
        <Skeleton className="h-32 w-full" />
      ) : last ? (
        <div className="border border-line rounded-lg p-5 mb-6">
          <div className="flex items-center justify-between mb-3">
            <span className="font-semibold">Build #{last.id}</span>
            <Badge variant="positive">{last.status}</Badge>
          </div>
          <div className="text-xs text-muted">
            <span className="font-mono">{last.out_dir ?? "—"}</span>
            {last.page_count != null && (
              <span className="ml-3">· {last.page_count} pages</span>
            )}
          </div>
        </div>
      ) : (
        <div className="border border-line rounded-lg p-8 text-center text-muted text-sm mb-6">
          No builds yet. Trigger your first build.
        </div>
      )}

      {last && (
        <div className="grid grid-cols-4 gap-3 text-xs mb-6">
          <div>
            <div className="text-muted">Build ID</div>
            <div className="font-mono">{(last as any).build_id ?? "—"}</div>
          </div>
          <div>
            <div className="text-muted">Theme</div>
            <div>{(last as any).theme_id ?? "—"}</div>
          </div>
          <div>
            <div className="text-muted">Deployment base</div>
            <div className="font-mono">{(last as any).deployment_base ?? "/"}</div>
          </div>
          <div>
            <div className="text-muted">Asset rev</div>
            <div className="font-mono">{(last as any).asset_revision ?? "—"}</div>
          </div>
        </div>
      )}

      <h2 className="text-sm font-semibold text-foreground mb-3">Build History</h2>
      <div className="border border-line rounded-lg overflow-hidden mb-6">
        <div className="flex bg-surface/50 text-xs font-semibold text-muted uppercase tracking-wider">
          <div className="flex-1 px-4 py-2.5">Build</div>
          <div className="w-24 px-4 py-2.5">Pages</div>
          <div className="w-24 px-4 py-2.5">Status</div>
          <div className="w-40 px-4 py-2.5">Time</div>
        </div>
        {buildsQ.isLoading ? (
          <div className="p-4">
            <Skeleton className="h-6 w-full" />
          </div>
        ) : builds.length > 0 ? (
          builds.map((b) => (
            <div key={b.id} className="flex border-t border-line text-sm">
              <div className="flex-1 px-4 py-2.5">#{b.id}</div>
              <div className="w-24 px-4 py-2.5 text-muted">{b.page_count ?? "—"}</div>
              <div className="w-24 px-4 py-2.5">
                <Badge variant="positive">{b.status}</Badge>
              </div>
              <div className="w-40 px-4 py-2.5 text-muted text-xs">{b.created_at}</div>
            </div>
          ))
        ) : (
          <div className="p-8 text-center text-muted text-sm">No build history</div>
        )}
      </div>

      <h2 className="text-sm font-semibold text-foreground mb-3">
        Deployments
        {deploys[0] && (
          <Badge variant={deploys[0].status === "failed" ? "warning" : "positive"} className="ml-2">
            {deploys[0].status}
          </Badge>
        )}
      </h2>
      <div className="border border-line rounded-lg overflow-hidden">
        <div className="flex bg-surface/50 text-xs font-semibold text-muted uppercase tracking-wider">
          <div className="flex-1 px-4 py-2.5">Repo</div>
          <div className="w-24 px-4 py-2.5">Status</div>
          <div className="w-32 px-4 py-2.5">Commit</div>
          <div className="w-40 px-4 py-2.5">Started</div>
        </div>
        {deploysQ.isLoading ? (
          <div className="p-4">
            <Skeleton className="h-6 w-full" />
          </div>
        ) : deploys.length > 0 ? (
          deploys.map((d) => (
            <div key={d.run_id} className="flex border-t border-line text-sm">
              <div className="flex-1 px-4 py-2.5">
                {d.owner}/{d.repo}
                {d.url && (
                  <a href={d.url} target="_blank" rel="noreferrer" className="ml-2 text-xs underline">
                    Open site ↗
                  </a>
                )}
              </div>
              <div className="w-24 px-4 py-2.5">
                <Badge variant={d.status === "failed" ? "warning" : "positive"}>{d.status}</Badge>
              </div>
              <div className="w-32 px-4 py-2.5 font-mono text-xs">
                {d.commit_sha?.slice(0, 8) ?? "—"}
              </div>
              <div className="w-40 px-4 py-2.5 text-muted text-xs">{d.started_at}</div>
            </div>
          ))
        ) : (
          <div className="p-8 text-center text-muted text-sm">No deployments yet</div>
        )}
      </div>
    </div>
  );
}
