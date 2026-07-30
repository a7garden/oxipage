// SetupGuard — setup 모드 감지 + 리다이렉트 (doc/13 §13.7.4)

import { useEffect, useState } from "react";
import { Navigate } from "react-router";
import { fetchSetupStatus, SetupStatus, SetupCompletedError } from "./api";

interface Props {
  children: React.ReactNode;
  /** If true, loading state uses min-h-screen (standalone pages). If false, uses py-20 (inside a shell layout). */
  fullPage?: boolean;
}

/**
 * SetupGuard:
 * - /setup/* 경로에서 setup_mode=false면 / 로 리다이렉트
 * - 그 외 경로에서 setup_mode=true면 /setup 으로 리다이렉트
 */
export function SetupGuard({ children, fullPage = true }: Props) {
  const [status, setStatus] = useState<SetupStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [isSetup, setIsSetup] = useState(false);

  useEffect(() => {
    const path = window.location.pathname;
    setIsSetup(path.startsWith("/setup"));

    fetchSetupStatus()
      .then((s) => {
        setStatus(s);
        setLoading(false);
      })
      .catch((err) => {
        if (err instanceof SetupCompletedError) {
          setStatus({ setup_mode: false });
        } else {
          console.warn("setup status check failed:", err);
          setStatus({ setup_mode: false });
        }
        setLoading(false);
      });
  }, []);

  if (loading) {
    return (
      <div className={fullPage ? "min-h-screen flex items-center justify-center" : "py-20 flex items-center justify-center"}>
        <div className="animate-pulse text-subtle">Loading...</div>
      </div>
    );
  }

  if (!status) return null;

  // On /setup/* paths — redirect to / if setup is done
  if (isSetup && !status.setup_mode) {
    return <Navigate to="/" replace />;
  }

  // On non-setup paths — redirect to /setup if setup is needed
  if (!isSetup && status.setup_mode) {
    return <Navigate to="/setup" replace />;
  }

  return <>{children}</>;
}
