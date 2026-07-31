import { Component, type ErrorInfo, type ReactNode } from "react";

interface Props {
  children: ReactNode;
}

interface State {
  hasError: boolean;
  error: Error | null;
  category: "render" | "chunk-load" | "unknown";
}

/** Compiled SPA revision, injected into admin.html by serve_asset as a meta tag. */
function getSpaRevision(): string {
  return (
    document.querySelector('meta[name="oxipage-spa-revision"]')?.getAttribute("content") ??
    "unknown"
  );
}

export class AdminErrorBoundary extends Component<Props, State> {
  state: State = { hasError: false, error: null, category: "unknown" };

  static getDerivedStateFromError(error: Error): State {
    const category: State["category"] = error.message.includes("Failed to fetch dynamically imported module")
      || error.message.includes("error loading dynamically imported module")
      ? "chunk-load"
      : "render";
    return { hasError: true, error, category };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("AdminErrorBoundary caught:", error, info);
  }

  handleReload = () => {
    window.location.reload();
  };

  handleClearCache = async () => {
    try {
      if ("serviceWorker" in navigator) {
        const regs = await navigator.serviceWorker.getRegistrations();
        for (const r of regs) await r.unregister();
      }
      if ("caches" in window) {
        const names = await caches.keys();
        for (const n of names) await caches.delete(n);
      }
    } catch {
      // Best-effort; reload regardless.
    }
    window.location.reload();
  };

  render() {
    if (!this.state.hasError) return this.props.children;

    const isChunk = this.state.category === "chunk-load";

    return (
      <div className="min-h-screen flex items-center justify-center bg-canvas p-8">
        <div className="max-w-md space-y-4 text-center">
          <h1 className="text-xl font-bold text-foreground">
            {isChunk ? "Console needs to reload" : "Console encountered an error"}
          </h1>
          <p className="text-sm text-muted">
            {isChunk
              ? "A cached version of the console is out of date. Reloading will fetch the latest build."
              : "An unexpected error occurred while rendering the console."}
          </p>
          {this.state.error && (
            <pre className="text-xs text-left bg-surface p-3 rounded border border-line overflow-auto max-h-32">
              {this.state.error.message}
            </pre>
          )}
          <div className="flex gap-2 justify-center">
            <button
              onClick={this.handleReload}
              className="px-4 py-2 text-sm font-medium rounded-md bg-primary text-primary-foreground hover:bg-primary/90"
            >
              Reload console
            </button>
            <button
              onClick={this.handleClearCache}
              className="px-4 py-2 text-sm font-medium rounded-md border border-line text-foreground hover:bg-surface"
            >
              Clear cache and reload
            </button>
          </div>
          <p className="text-xs text-muted">
            SPA revision: <code className="font-mono">{getSpaRevision().slice(0, 12)}</code>
          </p>
        </div>
      </div>
    );
  }
}
