/** Safe field access for unknown API response rows */
export function field(row: unknown, key: string): unknown {
  if (row && typeof row === "object" && key in row) {
    return (row as Record<string, unknown>)[key];
  }
  return undefined;
}

/** Coerce unknown to string with fallback */
export function str(v: unknown, fallback = "—"): string {
  return typeof v === "string" || typeof v === "number" ? String(v) : fallback;
}

/** Coerce unknown to number with fallback */
export function num(v: unknown, fallback = 0): number {
  return typeof v === "number" ? v : fallback;
}

/** Coerce unknown to boolean (truthy check) */
export function bool(v: unknown): boolean {
  return !!v;
}
