import { useEffect } from "react";
import { useLocation } from "react-router";

/**
 * Reset scroll position on route change. The console's scroll container is the
 * `<main>` element (overflow-auto), not the window, so both are reset.
 */
export function ScrollToTop() {
  const { pathname } = useLocation();
  useEffect(() => {
    window.scrollTo(0, 0);
    document.querySelector("main")?.scrollTo(0, 0);
  }, [pathname]);
  return null;
}
