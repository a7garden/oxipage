// @ts-expect-error — `bun:test` types ship with bun itself, not via a package;
// tsc complains because bun-types isn't a declared dep (not needed for `bun test`).
import { describe, expect, test } from "bun:test";
import type { MovieEntry } from "../../shared/api";
import { computeMovieStats } from "./computeMovieStats";

const fixtures: MovieEntry[] = [
  {
    id: 1, slug: "inception", tmdb_id: 27205, media_type: "movie",
    title: "인셉션", title_ko: "인셉션", title_en: "Inception",
    poster_path: "/a.jpg", release_year: 2010, runtime_min: 148, watched_at: null,
    rating: 9, review_ko: null, review_en: null, rewatch: 0,
    series_group_id: null, series_order: null, published_at: null,
    created_at: "2026-01-01", updated_at: "2026-01-01",
    genres: [{ name_en: "Sci-Fi", name_ko: "SF" }],
    cast: [{ id: 1, slug: "leo", name_en: "Leonardo DiCaprio", role: "actor" }],
    directors: [{ id: 2, slug: "nolan", name_en: "Christopher Nolan", role: "director" }],
  },
  {
    id: 2, slug: "parasite", tmdb_id: 496243, media_type: "movie",
    title: "기생충", title_ko: "기생충", title_en: "Parasite",
    poster_path: "/b.jpg", release_year: 2019, runtime_min: 132, watched_at: null,
    rating: 8, review_ko: null, review_en: null, rewatch: 1,
    series_group_id: null, series_order: null, published_at: null,
    created_at: "2026-02-01", updated_at: "2026-02-01",
    genres: [{ name_en: "Thriller", name_ko: "스릴러" }],
    cast: [
      { id: 3, slug: "song", name_en: "Song Kang-ho", role: "actor" },
      { id: 4, slug: "cho", name_en: "Cho Yeo-jeong", role: "actor" },
    ],
    directors: [{ id: 5, slug: "bong", name_en: "Bong Joon-ho", role: "director" }],
  },
  {
    id: 3, slug: "inception-again", tmdb_id: 999, media_type: "tv",
    title: "인셉션 TV", title_ko: "인셉션 TV", title_en: "Inception TV",
    poster_path: null, release_year: 2010, runtime_min: null, watched_at: null,
    rating: 0, review_ko: null, review_en: null, rewatch: 0,
    series_group_id: null, series_order: null, published_at: null,
    created_at: "2026-03-01", updated_at: "2026-03-01",
    genres: [{ name_en: "Sci-Fi", name_ko: "SF" }],
    cast: [{ id: 1, slug: "leo", name_en: "Leonardo DiCaprio", role: "actor" }],
    directors: [{ id: 2, slug: "nolan", name_en: "Christopher Nolan", role: "director" }],
  },
];

describe("computeMovieStats", () => {
  test("totals and counts", () => {
    const s = computeMovieStats(fixtures);
    expect(s.total).toBe(3);
    expect(s.actorCount).toBe(3); // DiCaprio, Song, Cho
    expect(s.directorCount).toBe(2); // Nolan, Bong
  });

  test("year span is contiguous", () => {
    const s = computeMovieStats(fixtures);
    expect(s.yearMin).toBe(2010);
    expect(s.yearMax).toBe(2019);
    // 2010..2019 = 10 rows; counts sum to number of known years (3 entries, 2 in 2010, 1 in 2019)
    expect(s.years.length).toBe(10);
    expect(s.years[0]).toEqual({ year: 2010, count: 2 });
    expect(s.years[9]).toEqual({ year: 2019, count: 1 });
  });

  test("genres tally by name_en with counts", () => {
    const s = computeMovieStats(fixtures);
    expect(s.genres).toContainEqual({ name: "Sci-Fi", count: 2 });
    expect(s.genres).toContainEqual({ name: "Thriller", count: 1 });
  });

  test("runtime: avg over non-null only; buckets sum to known runtimes", () => {
    const s = computeMovieStats(fixtures);
    expect(s.avgRuntime).toBe(140); // (148+132)/2
    const bucketSum = s.runtimeBuckets.reduce((n, b) => n + b.count, 0);
    expect(bucketSum).toBe(2); // only the 2 entries with runtime_min
  });

  test("rating mean ignores 0/absent ratings", () => {
    const s = computeMovieStats(fixtures);
    expect(s.ratingMean).toBe(8.5); // (9+8)/2, the rating 0 entry excluded
  });
});
