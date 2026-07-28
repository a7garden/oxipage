// Step 1: 사이트 기본 정보 (doc/13 §13.7.2)

import { useState } from "react";
import { Button } from "../shared/ui/button";
import { Input } from "../shared/ui/input";

interface Props {
  onNext: (data: { name: string; base_url?: string }) => void;
  loading: boolean;
}

export function StepSite({ onNext, loading }: Props) {
  const [name, setName] = useState("");
  const [lang, setLang] = useState<"ko" | "en">("ko");
  const valid = name.trim().length > 0 && name.trim().length <= 50;

  return (
    <div>
      <h2 className="text-xl font-semibold mb-6 text-center">사이트 이름</h2>

      <label className="block text-sm font-medium mb-2">사이트 이름</label>
      <Input
        placeholder="내 작업실"
        value={name}
        onChange={(e) => setName(e.target.value)}
        maxLength={50}
        autoFocus
      />
      <p className="text-xs text-subtle mt-1">{name.length}/50</p>

      <label className="block text-sm font-medium mt-6 mb-2">기본 언어</label>
      <div className="flex gap-2">
        <button
          onClick={() => setLang("ko")}
          className={`flex-1 py-2 px-4 rounded-md border text-sm transition-colors ${
            lang === "ko"
              ? "bg-primary text-primary-foreground border-primary"
              : "border-line hover:bg-surface"
          }`}
        >
          🇰🇷 한국어
        </button>
        <button
          onClick={() => setLang("en")}
          className={`flex-1 py-2 px-4 rounded-md border text-sm transition-colors ${
            lang === "en"
              ? "bg-primary text-primary-foreground border-primary"
              : "border-line hover:bg-surface"
          }`}
        >
          🇺🇸 English
        </button>
      </div>

      <div className="flex justify-end mt-8">
        <Button onClick={() => onNext({ name: name.trim() })} disabled={!valid || loading}>
          {loading ? "저장 중..." : "다음 →"}
        </Button>
      </div>
    </div>
  );
}
