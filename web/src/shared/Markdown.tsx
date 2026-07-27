import { useMemo } from 'react';
import MarkdownIt from 'markdown-it';

const md = new MarkdownIt({ linkify: true });

export function Markdown({ source }: { source: string }) {
  const html = useMemo(() => md.render(source), [source]);
  // 서버에 저장된 오너 본인의 마크다운이라 sanitize 없이 렌더링한다 (1인 오너 전제, doc §0.3).
  return <div className="markdown" dangerouslySetInnerHTML={{ __html: html }} />;
}
