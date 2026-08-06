// Generic client-side filter/sort spine for collection list pages.
// Owns the text-query + sort-key state and the filtered result. Page-specific
// facets (e.g. a status or genre dimension) are composed OUTSIDE this hook —
// pre-filter the array you pass in, or fold extra predicates into `matches`.
//
// Usage:
//   const { query, setQuery, sort, setSort, filtered } = useCollectionFilter(items, {
//     matches: (b, q) => b.title.toLowerCase().includes(q),
//     sortFns: { recent: (a, b) => b.created_at.localeCompare(a.created_at) },
//     initialSort: "recent",
//   });

import { useMemo, useState } from "react";

export type SortFns<T> = Record<string, (a: T, b: T) => number>;

export function useCollectionFilter<T>(
  items: T[] | undefined,
  opts: {
    matches: (item: T, query: string) => boolean;
    sortFns?: SortFns<T>;
    initialSort?: string;
  },
) {
  const [query, setQuery] = useState("");
  const [sort, setSort] = useState(opts.initialSort ?? "default");

  const filtered = useMemo(() => {
    if (!items) return [];
    const q = query.trim().toLowerCase();
    const list = q ? items.filter((it) => opts.matches(it, q)) : [...items];
    const sorter = opts.sortFns?.[sort];
    return sorter ? [...list].sort(sorter) : list;
  }, [items, query, sort, opts]);

  return {
    query,
    setQuery,
    sort,
    setSort,
    filtered,
    hasFilters: !!query,
    clearQuery: () => setQuery(""),
  };
}
