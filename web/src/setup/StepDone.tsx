// 완료 화면 — 토큰 표시 + 사이트 보기 / 관리 콘솔 (doc/13 §13.7.2)

import { useState } from "react";
import { Button } from "../shared/ui/button";
import type { CompleteResult } from "./api";

interface Props {
  result: CompleteResult;
}

export function StepDone({ result }: Props) {
  const [copied, setCopied] = useState(false);

  const copyToken = async () => {
    try {
      await navigator.clipboard.writeText(result.token);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // clipboard API not available
    }
  };

  return (
    <div className="text-center">
      <div className="text-4xl mb-4">🎉</div>
      <h2 className="text-xl font-semibold mb-2">설정 완료!</h2>
      <p className="text-sm text-subtle mb-6">
        CLI credentials 파일에 자동 저장되었습니다.
      </p>

      <div className="bg-surface border border-line rounded-lg p-4 mb-6 text-left">
        <div className="text-xs text-subtle mb-2">
          CLI 토큰 (한 번만 표시됩니다):
        </div>
        <div className="flex items-center gap-2">
          <code className="flex-1 text-xs font-mono bg-canvas border border-line rounded px-2 py-1 truncate">
            {result.token}
          </code>
          <button
            onClick={copyToken}
            className="shrink-0 text-xs px-2 py-1 rounded border border-line hover:bg-raised transition-colors"
          >
            {copied ? "✓" : "📋"}
          </button>
        </div>
      </div>

      <div className="flex gap-3 justify-center">
        <Button onClick={() => (window.location.href = "/")}>
          🏠 사이트 보기
        </Button>
        <Button
          variant="secondary"
          onClick={() => window.open("http://127.0.0.1:8788", "_blank")}
        >
          ⚙️ 관리 콘솔
        </Button>
      </div>
    </div>
  );
}
