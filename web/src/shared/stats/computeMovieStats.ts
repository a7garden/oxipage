// Movie stats, adapted to oxi's MovieEntry shape (NO nation field; genres/cast/
// directors carry localized name_en/name_ko). Ported from ../blog-test's
// movieStats.ts structure, canonicalized on name_en (TMDB-keyed). Localized in the page.

import type { MovieEntry } from "../../shared/api";

export interface MovieCountRow {
  name: string;
  count: number;
}

export interface MovieStats {
  total: number;
  directorCount: number;
  actorCount: number;
  yearMin: number;
  yearMax: number;
  avgRuntime: number;
  ratingMean: number;
  years: { year: number; count: number }[];
  genres: MovieCountRow[];
  actors: MovieCountRow[];
  directors: MovieCountRow[];
  runtimeBuckets: MovieCountRow[];
  ratingBuckets: MovieCountRow[];
}

function topRows(map: Map<string, number>, limit: number): MovieCountRow[] {
  return [...map.entries()]
    .map(([name, count]) => ({ name, count }))
    .sort((a, b) => b.count - a.count || a.name.localeCompare(b.name))
    .slice(0, limit);
}

function bucketKey(value: number, edges: number[]): string {
  for (let i = 0; i < edges.length; i++) {
    if (value < edges[i]) return i === 0 ? `< ${edges[0]}` : `${edges[i - 1]}–${edges[i]}`;
  }
  return `${edges[edges.length - 1]}+`;
}

export function computeMovieStats(movies: MovieEntry[]): MovieStats {
  const total = movies.length;
  const genreCounts = new Map<string, number>();
  const actorCounts = new Map<string, number>();
  const directorCounts = new Map<string, number>();
  const yearCounts = new Map<number, number>();
  let runtimeSum = 0;
  let runtimeN = 0;
  let ratingSum = 0;
  let ratingN = 0;

  for (const m of movies) {
    for (const g of m.genres) {
      const key = g.name_en;
      genreCounts.set(key, (genreCounts.get(key) ?? 0) + 1);
    }
    for (const p of m.cast) {
      actorCounts.set(p.name_en, (actorCounts.get(p.name_en) ?? 0) + 1);
    }
    for (const d of m.directors) {
      directorCounts.set(d.name_en, (directorCounts.get(d.name_en) ?? 0) + 1);
    }
    if (m.release_year != null) {
      yearCounts.set(m.release_year, (yearCounts.get(m.release_year) ?? 0) + 1);
    }
    if (m.runtime_min != null && m.runtime_min > 0) {
      runtimeSum += m.runtime_min;
      runtimeN += 1;
    }
    if (Number.isFinite(m.rating) && m.rating > 0) {
      ratingSum += m.rating;
      ratingN += 1;
    }
  }

  const years = [...yearCounts.keys()].filter((y): y is number => y != null);
  const yearMin = years.length ? Math.min(...years) : 0;
  const yearMax = years.length ? Math.max(...years) : 0;

  // Contiguous year span.
  const yearRows: { year: number; count: number }[] = [];
  if (years.length) {
    for (let y = yearMin; y <= yearMax; y++) {
      yearRows.push({ year: y, count: yearCounts.get(y) ?? 0 });
    }
  }

  // Runtime buckets: <90 / 90–120 / 120–150 / 150+
  const runtimeEdges = [90, 120, 150];
  const runtimeBuckets = new Map<string, number>();
  // Rating buckets: 0.5 step from 0..10
  const ratingBuckets = new Map<string, number>();
  for (const m of movies) {
    if (m.runtime_min != null && m.runtime_min > 0) {
      const k = bucketKey(m.runtime_min, runtimeEdges);
      runtimeBuckets.set(k, (runtimeBuckets.get(k) ?? 0) + 1);
    }
    if (Number.isFinite(m.rating) && m.rating > 0) {
      const lo = Math.floor(m.rating / 0.5) * 0.5;
      const label = lo % 1 === 0 ? `${lo.toFixed(0)}–${(lo + 0.5).toFixed(1)}` : `${lo.toFixed(1)}–${(lo + 0.5).toFixed(1)}`;
      ratingBuckets.set(label, (ratingBuckets.get(label) ?? 0) + 1);
    }
  }

  return {
    total,
    directorCount: directorCounts.size,
    actorCount: actorCounts.size,
    yearMin,
    yearMax,
    avgRuntime: runtimeN ? runtimeSum / runtimeN : 0,
    ratingMean: ratingN ? ratingSum / ratingN : 0,
    years: yearRows,
    genres: topRows(genreCounts, 15),
    actors: topRows(actorCounts, 15),
    directors: topRows(directorCounts, 10),
    runtimeBuckets: orderBuckets(runtimeBuckets, ["< 90", "90–120", "120–150", "150+"]),
    ratingBuckets: orderRatingBuckets(ratingBuckets),
  };
}

function orderBuckets(map: Map<string, number>, order: string[]): MovieCountRow[] {
  return order
    .filter((k) => map.has(k))
    .map((k) => ({ name: k, count: map.get(k)! }));
}

function orderRatingBuckets(map: Map<string, number>): MovieCountRow[] {
  // Order by the numeric low edge.
  return [...map.entries()]
    .map(([name, count]) => ({ name, count, lo: parseFloat(name) }))
    .sort((a, b) => a.lo - b.lo)
    .map(({ name, count }) => ({ name, count }));
}
