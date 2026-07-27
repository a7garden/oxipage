import { type ClassValue, clsx } from "clsx";
import { twMerge } from "tailwind-merge";

/** Merge conditional class names; Tailwind utilities win conflicts via tailwind-merge. */
export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}
