// Step 4: 프로필 (doc/13 §13.7.2)

import { useState } from "react";
import { Button } from "../shared/ui/button";
import { Input } from "../shared/ui/input";

interface Props {
  siteName: string;
  onNext: (data: {
    display_name?: string;
    tagline_ko?: string;
    tagline_en?: string;
    github_username?: string;
    bio_ko?: string;
    bio_en?: string;
  }) => void;
  onBack: () => void;
  loading: boolean;
}

export function StepProfile({ siteName, onNext, onBack, loading }: Props) {
  const [displayName, setDisplayName] = useState(siteName);
  const [taglineKo, setTaglineKo] = useState("");
  const [taglineEn, setTaglineEn] = useState("");
  const [github, setGithub] = useState("");

  return (
    <div>
      <h2 className="text-xl font-semibold mb-2 text-center">프로필 정보</h2>
      <p className="text-sm text-subtle text-center mb-6">선택 사항입니다. 건너뛰어도 됩니다.</p>

      <label className="block text-sm font-medium mb-2">표시 이름</label>
      <Input value={displayName} onChange={(e) => setDisplayName(e.target.value)} />

      <label className="block text-sm font-medium mt-4 mb-2">한 줄 소개 (한국어)</label>
      <Input value={taglineKo} onChange={(e) => setTaglineKo(e.target.value)} placeholder="개발자 & 작가" />

      <label className="block text-sm font-medium mt-4 mb-2">Tagline (English)</label>
      <Input value={taglineEn} onChange={(e) => setTaglineEn(e.target.value)} placeholder="Developer & Writer" />

      <label className="block text-sm font-medium mt-4 mb-2">GitHub 사용자명</label>
      <Input value={github} onChange={(e) => setGithub(e.target.value)} placeholder="username" />

      <div className="flex justify-between mt-8">
        <Button variant="secondary" onClick={onBack}>
          ← 이전
        </Button>
        <div className="flex gap-2">
          <Button
            variant="ghost"
            onClick={() => onNext({})}
            disabled={loading}
          >
            건너뛰기
          </Button>
          <Button
            onClick={() =>
              onNext({
                display_name: displayName,
                tagline_ko: taglineKo || undefined,
                tagline_en: taglineEn || undefined,
                github_username: github || undefined,
              })
            }
            disabled={loading}
          >
            {loading ? "저장 중..." : "다음 →"}
          </Button>
        </div>
      </div>
    </div>
  );
}
