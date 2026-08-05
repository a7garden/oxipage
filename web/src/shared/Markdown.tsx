import { useMemo } from "react";
import MarkdownIt from "markdown-it";
import { useAssetResolver } from "./asset-context";
import {
  deploymentBasePrefix,
  getImageManifest,
  isMediaRef,
  resolveMedia,
} from "./image-manifest";

export function Markdown({ source }: { source: string }) {
  const resolver = useAssetResolver();
  // 서버에 저장된 오너 본인의 마크다운이라 sanitize 없이 렌더링한다 (1인 오너 전제, doc §0.3).
  // The instance is rebuilt per resolver so the image rule closes over the
  // active context (admin endpoint vs document.baseURI) — creation is cheap.
  // `deploymentBasePrefix` is read once per `useMemo` call so the rule closes
  // over the value the build writer injected via `<base href>` at page load.
  const html = useMemo(() => {
    const md = new MarkdownIt({ linkify: true });
    const renderImage =
      md.renderer.rules.image ??
      ((tokens, idx, options, _env, self) => self.renderToken(tokens, idx, options));
    const base = deploymentBasePrefix();
    const manifest = getImageManifest();
    md.renderer.rules.image = (tokens, idx, opts, env, self) => {
      const src = tokens[idx].attrGet("src") ?? "";
      // Manifest hit first — when the loaded manifest has an entry for this
      // `media/...` ref, emit the optimized `<img>` (matching the Rust
      // prerender's attribute set: src/srcset/width/height/loading=lazy/
      // decoding=async/alt="") and bypass the default token render. Until
      // the manifest fetch lands `getImageManifest()` returns `{}` and this
      // branch never fires, so the existing behavior is preserved.
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
  }, [source, resolver]);
  return <div className="markdown" dangerouslySetInnerHTML={{ __html: html }} />;
}
