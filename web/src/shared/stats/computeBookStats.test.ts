// @ts-expect-error — `bun:test` types ship with bun itself, not via a package;
// tsc complains because bun-types isn't a declared dep (not needed for `bun test`).
import { describe, expect, test } from "bun:test";
import type { Book } from "../../shared/api";
import { computeBookStats, parseAuthorList, statusLabel } from "./computeBookStats";

const fixtures: Book[] = [
  {
    id: 1, source: "aladin", external_id: "a1", isbn13: "111", title: "사피엔스",
    author: "유발 하라리 (지은이)", cover_image_url: null, rating: 9,
    review_ko: null, review_en: null, status: "completed",
    started_at: null, finished_at: null, published_at: "2015-01-01",
    created_at: "2026-01-01", updated_at: "2026-01-01",
    category: "인문", publisher: "김영사", page_count: 500,
  },
  {
    id: 2, source: "aladin", external_id: "a2", isbn13: "222", title: "21세기 자본",
    author: "토마 피케티 (지은이), 장석준 (옮긴이)", cover_image_url: null, rating: 0,
    review_ko: null, review_en: null, status: "reading",
    started_at: null, finished_at: null, published_at: "2014-02-01",
    created_at: "2026-02-01", updated_at: "2026-02-01",
    category: "경제", publisher: "자음과모음", page_count: 800,
  },
  {
    id: 3, source: "manual", external_id: null, isbn13: null, title: "소설",
    author: "유발 하라리; 김작가", cover_image_url: null, rating: 7,
    review_ko: null, review_en: null, status: "wishlist",
    started_at: null, finished_at: null, published_at: null,
    created_at: "2026-03-01", updated_at: "2026-03-01",
    category: "소설", publisher: null, page_count: 200,
  },
  {
    id: 4, source: "aladin", external_id: "a4", isbn13: "444", title: "짧은 책",
    author: "단편가", cover_image_url: null, rating: 8,
    review_ko: null, review_en: null, status: "completed",
    started_at: null, finished_at: null, published_at: "2020-01-01",
    created_at: "2026-04-01", updated_at: "2026-04-01",
    category: "인문", publisher: "김영사", page_count: null,
  },
];

describe("parseAuthorList", () => {
  test("strips (지은이)/(옮긴이) and splits on comma/semicolon", () => {
    expect(parseAuthorList("유발 하라리 (지은이)")).toEqual(["유발 하라리"]);
    expect(parseAuthorList("토마 피케티 (지은이), 장석준 (옮긴이)")).toEqual([
      "토마 피케티",
      "장석준",
    ]);
    expect(parseAuthorList("유발 하라리; 김작가")).toEqual(["유발 하라리", "김작가"]);
    expect(parseAuthorList(null)).toEqual([]);
  });
});

describe("computeBookStats", () => {
  test("totals and author dedup", () => {
    const s = computeBookStats(fixtures);
    expect(s.total).toBe(4);
    expect(s.authorCount).toBe(5); // 하라리, 피케티, 장석준, 김작가, 단편가
    expect(s.authors).toContainEqual({ name: "유발 하라리", count: 2 });
  });

  test("status distribution", () => {
    const s = computeBookStats(fixtures);
    expect(s.byStatus).toContainEqual({ name: "completed", count: 2 });
    expect(s.byStatus).toContainEqual({ name: "reading", count: 1 });
    expect(s.byStatus).toContainEqual({ name: "wishlist", count: 1 });
  });

  test("rating mean excludes 0; year falls back to created_at", () => {
    const s = computeBookStats(fixtures);
    expect(s.ratingMean).toBe(8); // (9+7+8)/3
    // published_at years: 2015, 2014, 2020; book 3 has no published_at → created_at 2026
    expect(s.yearMin).toBe(2014);
    expect(s.yearMax).toBe(2026);
  });

  test("categories from non-null Book.category, sorted by count desc", () => {
    const s = computeBookStats(fixtures);
    expect(s.categories).toEqual([
      { name: "인문", count: 2 },
      { name: "경제", count: 1 },
      { name: "소설", count: 1 },
    ]);
  });

  test("publishers from Book.publisher (null excluded)", () => {
    const s = computeBookStats(fixtures);
    expect(s.publishers).toEqual([
      { name: "김영사", count: 2 },
      { name: "자음과모음", count: 1 },
    ]);
  });

  test("pageBuckets with edges [300, 500], only non-null page_count", () => {
    const s = computeBookStats(fixtures);
    // page_counts: 500, 800, 200, null
    //   500 → 500+ (500 < 500 is false)
    //   800 → 500+
    //   200 → < 300
    // 300–500 has 0 entries and is omitted.
    expect(s.pageBuckets).toEqual([
      { name: "< 300", count: 1 },
      { name: "500+", count: 2 },
    ]);
  });
});

describe("statusLabel", () => {
  test("localizes known statuses", () => {
    expect(statusLabel("completed", true)).toBe("읽음");
    expect(statusLabel("completed", false)).toBe("Completed");
    expect(statusLabel("custom", true)).toBe("custom");
  });
});
