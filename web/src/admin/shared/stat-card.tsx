import { cn } from "../../shared/ui/cn";

type ChangeTone = "positive" | "negative" | "warning" | "neutral";

interface StatCardProps {
  label: string;
  value: string | number;
  /** Free-form change text (e.g. "draft", "owner/repo"). */
  change?: string;
  /** Visual tone for the change footnote. Defaults to positive (legacy v1). */
  changeTone?: ChangeTone;
  className?: string;
}

const changeToneClass: Record<ChangeTone, string> = {
  positive: "text-positive-fg",
  negative: "text-destructive-fg",
  warning: "text-warning-fg",
  neutral: "text-muted",
};

/**
 * Dashboard stat card — smallest unit on the overview page. Surface is
 * `bg-raised` on top of the canvas so it lifts slightly under the elevation
 * system; the change footnote color is now requested by intent, not hex.
 */
export function StatCard({
  label,
  value,
  change,
  changeTone = "positive",
  className,
}: StatCardProps) {
  return (
    <div
      className={cn(
        "border border-line rounded-lg p-4 bg-raised",
        className,
      )}
    >
      <div className="text-xs text-muted mb-1">{label}</div>
      <div className="text-2xl font-bold text-foreground">{value}</div>
      {change && (
        <div className={cn("text-xs mt-0.5", changeToneClass[changeTone])}>
          {change}
        </div>
      )}
    </div>
  );
}
