// Shared client-side validators. These mirror the server rules in
// crates/oxipage-core/src/validation.rs and the spec's "Validation contract"
// section. Server is authoritative; client feedback is best-effort UX, never
// authoritative.

export function isHttpUrl(v: string): boolean {
  return /^https?:\/\//i.test(v);
}

export function isMediaPath(v: string): boolean {
  if (v.startsWith("/") || v.startsWith(".") || v.includes("..")) return false;
  if (v.startsWith("javascript:") || v.startsWith("data:") || v.startsWith("file:"))
    return false;
  return /^media\/[a-z0-9_-]+\/[a-z0-9._-]+$/i.test(v);
}

export function isImageValue(v: string): boolean {
  return isHttpUrl(v) || isMediaPath(v);
}

export function clampRating(v: unknown): { value: number | null; error?: string } {
  const n = typeof v === "number" ? v : Number(v);
  if (!Number.isFinite(n) || !Number.isInteger(n))
    return { value: null, error: "Rating must be an integer" };
  if (n < 0 || n > 10) return { value: null, error: "Rating must be between 0 and 10" };
  return { value: n };
}

export function validateYear(v: unknown): { value: number | null; error?: string } {
  const n = typeof v === "number" ? v : Number(v);
  if (!Number.isFinite(n) || !Number.isInteger(n))
    return { value: null, error: "Year must be an integer" };
  if (n < 1000 || n > 9999) return { value: null, error: "Year must be a 4-digit value" };
  return { value: n };
}

export function validateDateRange(start: string, end: string): string | null {
  if (!start || !end) return null;
  if (start > end) return "End date must not precede start date";
  return null;
}

export function validateEmail(v: string): string | null {
  if (!v) return null;
  // Pragmatic address check; server re-validates.
  if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(v)) return "Email is not valid";
  return null;
}

/// ISBN-13: 13 digits, last digit is the checksum.
export function validateIsbn13(v: string): string | null {
  if (!v) return null;
  const s = v.replace(/-/g, "");
  if (!/^\d{13}$/.test(s)) return "ISBN-13 must be 13 digits";
  let sum = 0;
  for (let i = 0; i < 12; i++) {
    const d = Number(s[i]);
    sum += i % 2 === 0 ? d : d * 3;
  }
  const check = (10 - (sum % 10)) % 10;
  if (check !== Number(s[12])) return "ISBN-13 checksum is invalid";
  return null;
}