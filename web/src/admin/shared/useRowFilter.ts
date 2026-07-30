import { useMemo, useState, useEffect } from "react";

/** Debounced search query value. */
function useDebounce(value: string, delay: number): string {
  const [debounced, setDebounced] = useState(value);
  useEffect(() => {
    const id = setTimeout(() => setDebounced(value), delay);
    return () => clearTimeout(id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [value, delay]);
  return debounced;
}

/**
 * Client-side row filter with debounced text matching.
 *
 * @param rows  Full list to filter.
 * @param query  Raw (non-debounced) search input.
 * @param keys  Function returning one or more string fields per row to search.
 * @param delay  Debounce ms (default 150).
 * @returns  Filtered list — full list when query is empty.
 */
export function useRowFilter<T>(
  rows: T[],
  query: string,
  keys: (row: T) => string[],
  delay = 150,
): T[] {
  const debounced = useDebounce(query, delay);

  return useMemo(() => {
    if (!debounced.trim()) return rows;
    const q = debounced.toLowerCase();
    return rows.filter((row) =>
      keys(row).some((v) => String(v).toLowerCase().includes(q)),
    );
    // `keys` is expected to be stable (defined at module or callback level).
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [rows, debounced]);
}
