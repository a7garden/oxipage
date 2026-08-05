import { useEffect, useMemo, useState } from "react";
import MarkdownIt from "markdown-it";
import { useAssetResolver } from "./asset-context";
import {
  deploymentBasePrefix,
  getImageManifest,
  isMediaRef,
  loadImageManifest,
  resolveMedia,
  type ImageManifest,
} from "./image-manifest";

export function Markdown({ source }: { source: string }) {
  const resolver = useAssetResolver();
  // React-tracked image manifest. The module-level `manifest` cache in
  // `image-manifest.ts` is still populated at module load for the markdown-it
  // rule's synchronous read path, but holding the value in component state
  // guarantees a late-arriving manifest re-renders this component. Without
  // the state, the module-var mutation was invisible to React and a post that
  // renders before the manifest fetch lands would silently fall back to the
  // non-optimized `<img>` path — JS users would see raw `media/...` URLs
  // instead of the WebP variants the prerendered HTML promised.
  const [manifest, setManifest] = useState<ImageManifest>(() =>
    getImageManifest(),
  );
  useEffect(() => {
    let cancelled = false;
    loadImageManifest().then((m) => {
      if (!cancelled) setManifest(m);
    });
    return () => {
      cancelled = true;
    };
  }, []);

  // 서버에 저장된 오너 본인의 마크다운이라 sanitize 없이 렌더링한다 (1인 오너 전제, doc §0.3).
  // The instance is rebuilt per resolver so the image rule closes over the
  // active context (admin endpoint vs document.baseURI) — creation is cheap.
  // `deploymentBasePrefix` is read once per `useMemo` call so the rule closes
  // over the value the build writer injected via `<base href>` at page load.
  // `manifest` is a dep so a late-arriving manifest re-runs the rule and
  // substitutes the optimized `<img>` for any already-rendered post body.
  const html = useMemo(() => {
    const md = new MarkdownIt({ linkify: true });
    const renderImage =
      md.renderer.rules.image ??
      ((tokens, idx, options, _env, self) => self.renderToken(tokens, idx, options));
    const base = deploymentBasePrefix();
    md.renderer.rules.image = (tokens, idx, opts, env, self) => {
      const src = tokens[idx].attrGet("src") ?? "";
      // Manifest hit first — when the loaded manifest has an entry for this
      // `media/...` ref, emit the optimized `<img>` (matching the Rust
      // prerender's attribute set: src/srcset/width/height/loading=lazy/
      // decoding=async/alt="") and bypass the default token render. Until
      // the manifest fetch lands the state defaults to `{}` and this branch
      // never fires, so the existing behavior is preserved.
      if (isMediaRef(src)) {
        const optimized = resolveMedia(src, base, manifest);
        if (optimized !== null) return optimized;
        // No manifest entry — fall back to the context-aware resolver and
        // let markdown-it render the default `<img>` token. External URLs
        // pass through `isMediaRef` as false and skip both branches.
        const resolved = resolver.resolve(src);
        if (resolved) tokens[idx].attrSet("src", resolved);
      }
      return renderImage(tokens, idx, opts, env, self);
    };
    return md.render(source);
  }, [source, resolver, manifest]);
  return <div className="markdown" dangerouslySetInnerHTML={{ __html: html }} />;
}