interface StatCardProps {
  label: string;
  value: string | number;
  change?: string;
  changeColor?: string;
}

export function StatCard({ label, value, change, changeColor }: StatCardProps) {
  return (
    <div className="border border-line rounded-lg p-4 bg-surface/30">
      <div className="text-xs text-muted mb-1">{label}</div>
      <div className="text-2xl font-bold text-foreground">{value}</div>
      {change && (
        <div className="text-xs mt-0.5" style={{ color: changeColor ?? "#22c55e" }}>
          {change}
        </div>
      )}
    </div>
  );
}
