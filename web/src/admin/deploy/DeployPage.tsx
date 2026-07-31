import { useParams } from "react-router";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";
import {
  listBuilds,
  triggerBuild,
  triggerDeploy,
  buildStreamUrl,
  deployStreamUrl,
  previewUrl,
  OperationConflictError,
  type BuildRecord,
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

interface BuildEvent {
  event: string;
  total?: number;
  ext_id?: string;
  pages?: number;
  total_pages?: number;
  count?: number;
  url?: string;
  error?: string;
  [k: string]: unknown;
}

const TERMINAL_EVENTS: Record<string, true> = {
  build_complete: true,
  build_failed: true,
  deployed: true,
  failed: true,
};

function formatEvent(ev: BuildEvent): LogLine {
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
    case "gh_check":
      return { text: "▶ Checking GitHub CLI…", tone: "info" };
    case "auth_check":
      return { text: "▶ Verifying authentication…", tone: "info" };
    case "worktree_ready":
      return { text: "✓ Prepared gh-pages worktree", tone: "ok" };
    case "files_copied":
      return { text: `✓ Copied ${ev.count} files`, tone: "ok" };
    case "pushing":
      return { text: "▶ Pushing to gh-pages…", tone: "info" };
    case "deployed":
      return { text: `✓ Deployed: ${ev.url}`, tone: "ok" };
    case "failed":
      return { text: `✗ Deploy failed: ${ev.error}`, tone: "err" };
    default:
      return { text: JSON.stringify(ev), tone: "info" };
  }
}

export function DeployPage() {
  const { slug } = useParams<{ slug: string }>()!;
  const qc = useQueryClient();
  const [lines, setLines] = useState<LogLine[]>([]);
  const [status, setStatus] = useState<RunStatus>("idle");
  const [op, setOp] = useState<OpKind | null>(null);
  const esRef = useRef<EventSource | null>(null);

  const { data, isLoading, isError } = useQuery({
    queryKey: ["site", slug, "builds"],
    queryFn: () => listBuilds(slug!),
    enabled: !!slug,
  });

  // Close any open SSE stream on unmount.
  useEffect(() => () => esRef.current?.close(), []);

  const builds: BuildRecord[] = data?.data ?? [];
  const last = builds[0];
  const busy = status === "running";

  function attachStream(kind: OpKind, id: string) {
    esRef.current?.close();
    setStatus("running");
    setOp(kind);
    const url =
      kind === "build" ? buildStreamUrl(slug!, id) : deployStreamUrl(slug!, id);
    const es = new EventSource(url);
    esRef.current = es;
    es.onmessage = (e) => {
      try {
        const ev = JSON.parse(e.data) as BuildEvent;
        setLines((prev) => [...prev, formatEvent(ev)]);
        if (TERMINAL_EVENTS[ev.event]) {
          const failed =
            ev.event === "build_failed" || ev.event === "failed";
          setStatus(failed ? "failed" : "done");
          es.close();
          esRef.current = null;
          if (kind === "build") {
            qc.invalidateQueries({ queryKey: ["site", slug, "builds"] });
          }
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
            Build static site and deploy to production
          </p>
        </div>
        <div className="flex gap-2">
          <Button variant="outline" onClick={onBuild} disabled={busy}>
            {busy && op === "build" ? "Building…" : "↧ Build"}
          </Button>
          <Button
            variant="outline"
            onClick={() => window.open(previewUrl(slug!), "_blank", "noopener,noreferrer")}
            disabled={!last || last.status !== "built"}
            title={
              !last
                ? "Run a build to enable preview"
                : last.status !== "built"
                  ? "Last build did not succeed"
                  : "Open the built site in a new tab"
            }
          >
            Preview Site ↗
          </Button>
          <Button onClick={onDeploy} disabled={busy}>
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

      <h2 className="text-sm font-semibold text-foreground mb-3">Last Build</h2>
      {isLoading ? (
        <Skeleton className="h-32 w-full" />
      ) : last ? (
        <div className="border border-line rounded-lg p-5 mb-6">
          <div className="flex items-center justify-between mb-3">
            <span className="font-semibold">Build #{last.id}</span>
            <Badge variant="positive">{last.status}</Badge>
          </div>
          <div className="flex gap-2 items-center text-xs text-muted mb-4">
            <span className="flex items-center gap-1">
              <span className="size-2 rounded-full bg-[#22c55e]" /> Build
            </span>
            <span className="text-line">→</span>
            <span className="flex items-center gap-1">
              <span className="size-2 rounded-full bg-[#22c55e]" /> Generate
            </span>
            <span className="text-line">→</span>
            <span className="flex items-center gap-1">
              <span className="size-2 rounded-full bg-[#22c55e]" /> Deploy
            </span>
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
      <div className="border border-line rounded-lg overflow-hidden">
        <div className="flex bg-surface/50 text-xs font-semibold text-muted uppercase tracking-wider">
          <div className="flex-1 px-4 py-2.5">Build</div>
          <div className="w-24 px-4 py-2.5">Pages</div>
          <div className="w-24 px-4 py-2.5">Status</div>
          <div className="w-40 px-4 py-2.5">Time</div>
        </div>
        {isLoading ? (
          <div className="p-4">
            <Skeleton className="h-6 w-full" />
          </div>
        ) : isError ? (
          <div className="p-8 text-center text-muted text-sm">
            Failed to load build history.{" "}
            <button
              onClick={() => qc.invalidateQueries({ queryKey: ["site", slug, "builds"] })}
              className="underline"
            >
              Retry
            </button>
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
    </div>
  );
}
