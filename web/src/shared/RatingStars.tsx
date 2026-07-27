import * as React from "react";

interface RatingStarsProps {
  /** 서버 저장값 0~10 정수 (0.5점 단위 = 정수 1스텝). /2 → 0.0~5.0 별 5개. */
  value: number;
  /** 읽기 전용 표시용이면 false, 입력용이면 true (현재 표시 전용). */
  readOnly?: boolean;
  size?: "sm" | "md" | "lg";
  ariaLabel?: string;
}

const SIZE_PX: Record<NonNullable<RatingStarsProps["size"]>, number> = {
  sm: 14,
  md: 18,
  lg: 24,
};

/**
 * 별점 5개 컴포넌트 (doc/02 §2.1, doc/03 토큰 --color-rating-fill → text-star).
 * value는 서버 원시값(0~10)이며 /2.0으로 0.0~5.0으로 환산해 반별까지 표시.
 * 자체 완결(self-contained) — 외부 CSS 의존 없음.
 */
export function RatingStars({
  value,
  size = "md",
  ariaLabel,
}: RatingStarsProps) {
  const score = Math.max(0, Math.min(10, value)) / 2; // 0.0 ~ 5.0
  const px = SIZE_PX[size];

  return (
    <span
      role="img"
      aria-label={ariaLabel ?? `별점 ${score.toFixed(1)} / 5`}
      className="inline-flex gap-0.5 align-middle"
    >
      {[0, 1, 2, 3, 4].map((i) => {
        const fill = Math.max(0, Math.min(1, score - i)); // 0, 0.5, 1
        return (
          <span
            key={i}
            aria-hidden="true"
            className="relative inline-block leading-none"
            style={{ width: px, height: px, fontSize: px }}
          >
            <span className="inline-block leading-none text-subtle/50">★</span>
            <span
              className="absolute inset-0 inline-block overflow-hidden whitespace-nowrap leading-none text-star"
              style={{ width: `${fill * 100}%` }}
            >
              ★
            </span>
          </span>
        );
      })}
    </span>
  );
}
