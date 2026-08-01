import { useMemo } from "react";
import MarkdownIt from "markdown-it";
import { useAssetResolver } from "./asset-context";

/** True for `media/...` logical references (with or without a leading slash). */
function isMediaRef(src: string): boolean {
  return /^\/?media\//.test(src.trim());
}

export function Markdown({ source }: { source: string }) {
  const resolver = useAssetResolver();
  // 서버에 저장된 오너 본인의 마크다운이라 sanitize 없이 렌더링한다 (1인 오너 전제, doc §0.3).
  // The instance is rebuilt per resolver so the image rule closes over the
  // active context (admin endpoint vs document.baseURI) — creation is cheap.
  const html = useMemo(() => {
    const md = new MarkdownIt({ linkify: true });
    const renderImage =
      md.renderer.rules.image ??
      ((tokens, idx, options, _env, self) => self.renderToken(tokens, idx, options));
    md.renderer.rules.image = (tokens, idx, opts, env, self) => {
      const src = tokens[idx].attrGet("src") ?? "";
      // Resolve only media logical paths; leave external URLs/anchors untouched.
      if (isMediaRef(src)) {
        const resolved = resolver.resolve(src);
        if (resolved) tokens[idx].attrSet("src", resolved);
      }
      return renderImage(tokens, idx, opts, env, self);
    };
    return md.render(source);
  }, [source, resolver]);
  return <div className="markdown" dangerouslySetInnerHTML={{ __html: html }} />;
}
