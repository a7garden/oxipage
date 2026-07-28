// Step 6: 샘플 콘텐츠 & API 키 (doc/13 §13.7.2)

import { useState } from "react";
import { Button } from "../shared/ui/button";
import { Input } from "../shared/ui/input";

interface Props {
  onNext: (data: {
    sample_post?: boolean;
    tmdb_key?: string;
    aladin_key?: string;
  }) => void;
  onBack: () => void;
  loading: boolean;
  onFinish: () => void;
}

export function StepContent({ onNext, onBack, loading, onFinish }: Props) {
  const [samplePost, setSamplePost] = useState(true);
  const [tmdbKey, setTmdbKey] = useState("");
  const [aladinKey, setAladinKey] = useState("");

  const handleFinish = () => {
    onNext({
      sample_post: samplePost,
      tmdb_key: tmdbKey || undefined,
      aladin_key: aladinKey || undefined,
    });
    // After content is saved, call complete
    onFinish();
  };

  return (
    <div>
      <h2 className="text-xl font-semibold mb-2 text-center">마지막으로</h2>
      <p className="text-sm text-subtle text-center mb-6">모두 선택 사항입니다</p>

      <button
        onClick={() => setSamplePost(!samplePost)}
        className={`flex items-center gap-3 w-full p-4 rounded-lg border text-left mb-6 transition-all ${
          samplePost ? "border-primary bg-primary/5" : "border-line"
        }`}
      >
        <div
          className={`w-5 h-5 rounded border-2 flex items-center justify-center shrink-0 ${
            samplePost ? "bg-primary border-primary" : "border-line"
          }`}
        >
          {samplePost && (
            <svg className="w-3 h-3 text-primary-foreground" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={3} d="M5 13l4 4L19 7" />
            </svg>
          )}
        </div>
        <div>
          <div className="text-sm font-medium">환영 글 생성하기</div>
          <div className="text-xs text-subtle">첫 방문자에게 보이는 샘플 블로그 글</div>
        </div>
      </button>

      <label className="block text-xs font-medium mb-2 text-subtle">외부 API 키 (선택)</label>

      <label className="block text-sm mb-1">TMDB (영화)</label>
      <Input
        value={tmdbKey}
        onChange={(e) => setTmdbKey(e.target.value)}
        placeholder="나중에 설정 → 관리 콘솔"
      />

      <label className="block text-sm mt-4 mb-1">알라딘 (책)</label>
      <Input
        value={aladinKey}
        onChange={(e) => setAladinKey(e.target.value)}
        placeholder="나중에 설정 가능"
      />

      <p className="text-xs text-subtle mt-2">나중에 관리 콘솔에서 추가할 수 있습니다</p>

      <div className="flex justify-between mt-8">
        <Button variant="secondary" onClick={onBack}>
          ← 이전
        </Button>
        <Button onClick={handleFinish} disabled={loading}>
          {loading ? "저장 중..." : "완료 ✓"}
        </Button>
      </div>
    </div>
  );
}
