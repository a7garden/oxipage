// Book stats, adapted to oxi's Book shape (NO category/publisher/pages; uses
// `author` string, `status`, `rating`, year from published_at/created_at).
// Ported from ../blog-test's bookStats.ts structure.

import type { Book } from "../../shared/api";

export interface BookCountRow {
  name: string;
  count: number;
}

export interface BookStats {
  total: number;
  authorCount: number;
  ratingMean: number;
  yearMin: number;
  yearMax: number;
  years: { year: number; count: number }[];
  authors: BookCountRow[];
  byStatus: BookCountRow[];
  ratingBuckets: BookCountRow[];
}

// Aladin-style "유발 하라리 (지은이), 조현욱 (옮긴이)" → ["유발 하라리", "조현욱"].
// Also tolerates semicolons and bare commas.
export function parseAuthorList(author: string | null | undefined): string[] {
  const raw = String(author ?? "").trim();
  if (!raw) return [];
  return raw
    .split(/[,;]/)
    .map((a) => a.replace(/\([^)]*\)/g, "").trim())
    .filter(Boolean);
}

function topRows(map: Map<string, number>, limit: number): BookCountRow[] {
  return [...map.entries()]
    .map(([name, count]) => ({ name, count }))
    .sort((a, b) => b.count - a.count || a.name.localeCompare(b.name))
    .slice(0, limit);
}

function parseYear(iso: string | null | undefined): number | null {
  if (!iso) return null;
  const m = /^(\d{4})/.exec(iso);
  return m ? Number(m[1]) : null;
}

export function computeBookStats(books: Book[]): BookStats {
  const total = books.length;
  const authorCounts = new Map<string, number>();
  const statusCounts = new Map<string, number>();
  const yearCounts = new Map<number, number>();
  const ratingBuckets = new Map<string, number>();
  let ratingSum = 0;
  let ratingN = 0;

  for (const b of books) {
    for (const a of parseAuthorList(b.author)) {
      authorCounts.set(a, (authorCounts.get(a) ?? 0) + 1);
    }
    if (b.status) {
      statusCounts.set(b.status, (statusCounts.get(b.status) ?? 0) + 1);
    }
    const year = parseYear(b.published_at) ?? parseYear(b.created_at);
    if (year != null) {
      yearCounts.set(year, (yearCounts.get(year) ?? 0) + 1);
    }
    if (Number.isFinite(b.rating) && b.rating > 0) {
      ratingSum += b.rating;
      ratingN += 1;
      const lo = Math.floor(b.rating / 0.5) * 0.5;
      const label = `${lo}–${lo + 0.5}`;
      ratingBuckets.set(label, (ratingBuckets.get(label) ?? 0) + 1);
    }
  }

  const years = [...yearCounts.keys()];
  const yearMin = years.length ? Math.min(...years) : 0;
  const yearMax = years.length ? Math.max(...years) : 0;
  const yearRows: { year: number; count: number }[] = [];
  if (years.length) {
    for (let y = yearMin; y <= yearMax; y++) {
      yearRows.push({ year: y, count: yearCounts.get(y) ?? 0 });
    }
  }

  return {
    total,
    authorCount: authorCounts.size,
    ratingMean: ratingN ? ratingSum / ratingN : 0,
    yearMin,
    yearMax,
    years: yearRows,
    authors: topRows(authorCounts, 15),
    byStatus: topRows(statusCounts, 10),
    ratingBuckets: [...ratingBuckets.entries()]
      .map(([name, count]) => ({ name, count, lo: parseFloat(name) }))
      .sort((a, b) => a.lo - b.lo)
      .map(({ name, count }) => ({ name, count })),
  };
}

// Localized status label for known statuses.
export function statusLabel(status: string, ko: boolean): string {
  const map: Record<string, [string, string]> = {
    wishlist: ["읽고 싶다", "Wishlist"],
    reading: ["읽는 중", "Reading"],
    completed: ["읽음", "Completed"],
    dropped: ["중단", "Dropped"],
  };
  const entry = map[status];
  return entry ? (ko ? entry[0] : entry[1]) : status;
}
