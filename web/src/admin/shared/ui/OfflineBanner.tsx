import { useState, useEffect } from "react";

/**
 * Top banner shown when the browser is offline. TanStack Query retries on
 * reconnect; this is a non-blocking visual hint.
 */
export function OfflineBanner() {
  const [online, setOnline] = useState(() =>
    typeof navigator !== "undefined" ? navigator.onLine : true,
  );
  useEffect(() => {
    const goOnline = () => setOnline(true);
    const goOffline = () => setOnline(false);
    window.addEventListener("online", goOnline);
    window.addEventListener("offline", goOffline);
    return () => {
      window.removeEventListener("online", goOnline);
      window.removeEventListener("offline", goOffline);
    };
  }, []);
  if (online) return null;
  return (
    <div className="bg-yellow-100 border-b border-yellow-300 px-4 py-1.5 text-xs text-yellow-800 text-center">
      Offline — changes will retry when reconnected
    </div>
  );
}
