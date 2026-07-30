import * as React from "react";
import { X } from "lucide-react";
import { cn } from "./cn";

interface DrawerProps {
  open: boolean;
  onClose: () => void;
  title: string;
  description?: string;
  children: React.ReactNode;
  footer?: React.ReactNode;
  width?: string; // tailwind class, e.g. "w-[500px]"
}

export function Drawer({
  open,
  onClose,
  title,
  description,
  children,
  footer,
  width = "w-[480px]",
}: DrawerProps) {
  React.useEffect(() => {
    if (!open) return;
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") onClose();
    }
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  if (!open) return null;
  return (
    <div className="fixed inset-0 z-50 flex">
      <div
        className="flex-1 bg-black/40"
        onClick={onClose}
        aria-hidden
      />
      <div
        className={cn(
          "bg-canvas border-l border-line h-full overflow-y-auto flex flex-col shadow-2xl",
          width,
        )}
        role="dialog"
        aria-modal="true"
        aria-label={title}
      >
        <div className="flex items-start justify-between px-5 py-4 border-b border-line">
          <div>
            <h2 className="text-base font-semibold text-foreground">{title}</h2>
            {description && (
              <p className="text-xs text-muted mt-0.5">{description}</p>
            )}
          </div>
          <button
            onClick={onClose}
            className="inline-flex items-center justify-center size-7 rounded-md text-muted hover:text-foreground hover:bg-surface/50"
            aria-label="Close"
          >
            <X size={16} />
          </button>
        </div>
        <div className="flex-1 px-5 py-4 overflow-y-auto">{children}</div>
        {footer && (
          <div className="px-5 py-4 border-t border-line flex justify-end gap-2 bg-surface/30">
            {footer}
          </div>
        )}
      </div>
    </div>
  );
}

interface DrawerFieldProps {
  label: string;
  hint?: string;
  required?: boolean;
  children: React.ReactNode;
}

export function DrawerField({ label, hint, required, children }: DrawerFieldProps) {
  return (
    <div className="mb-4">
      <label className="block text-xs font-semibold text-foreground mb-1.5">
        {label}
        {required && <span className="text-red-500 ml-1">*</span>}
      </label>
      {children}
      {hint && <p className="text-xs text-muted mt-1">{hint}</p>}
    </div>
  );
}
