// Step 3: 확장 선택 (doc/13 §13.7.2)

import { useState } from "react";
import { Button } from "../shared/ui/button";
import { Badge } from "../shared/ui/badge";
import type { ExtensionInfo } from "./api";

interface Props {
  extensions: ExtensionInfo[];
  onNext: (data: { enabled: string[] }) => void;
  onBack: () => void;
  loading: boolean;
}

const PRESETS: Record<string, string[]> = {
  전체: ["profile", "blog", "projects", "links", "novels", "movies", "books", "scraps", "activity"],
  콘텐츠: ["profile", "blog", "projects", "links"],
  최소: ["profile"],
};

export function StepExtensions({ extensions, onNext, onBack, loading }: Props) {
  const [selected, setSelected] = useState<Set<string>>(
    new Set(["profile", "blog", "projects", "links"]),
  );

  const toggle = (id: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  return (
    <div>
      <h2 className="text-xl font-semibold mb-2 text-center">활성화할 확장</h2>
      <p className="text-sm text-subtle text-center mb-6">나중에 설정에서 변경할 수 있습니다</p>

      <div className="flex gap-2 justify-center mb-6">
        {Object.entries(PRESETS).map(([label, ids]) => (
          <button
            key={label}
            onClick={() => setSelected(new Set(ids))}
            className="px-3 py-1 text-xs rounded-full border border-line hover:bg-surface transition-colors"
          >
            {label}
          </button>
        ))}
      </div>

      <div className="grid grid-cols-2 gap-3">
        {extensions.map((ext) => {
          const active = selected.has(ext.id);
          return (
            <button
              key={ext.id}
              onClick={() => toggle(ext.id)}
              className={`flex items-center gap-3 p-3 rounded-lg border text-left transition-all ${
                active
                  ? "border-primary bg-primary/5"
                  : "border-line hover:bg-surface"
              }`}
            >
              <div
                className={`w-5 h-5 rounded border-2 flex items-center justify-center shrink-0 transition-colors ${
                  active
                    ? "bg-primary border-primary"
                    : "border-line"
                }`}
              >
                {active && (
                  <svg className="w-3 h-3 text-primary-foreground" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={3} d="M5 13l4 4L19 7" />
                  </svg>
                )}
              </div>
              <div>
                <div className="text-sm font-medium">{ext.display_name.ko}</div>
                <div className="text-xs text-subtle">{ext.display_name.en}</div>
              </div>
            </button>
          );
        })}
      </div>

      <div className="flex justify-between mt-8">
        <Button variant="secondary" onClick={onBack}>
          ← 이전
        </Button>
        <Button onClick={() => onNext({ enabled: Array.from(selected) })} disabled={loading}>
          {loading ? "저장 중..." : "다음 →"}
        </Button>
      </div>
    </div>
  );
}
