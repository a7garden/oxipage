import * as React from "react";
import { cn } from "./cn";

interface CardProps extends React.ComponentProps<"div"> {
  title?: string;
  subtitle?: string;
}

function Card({ className, title, subtitle, children, ...props }: CardProps) {
  return (
    <div
      data-slot="card"
      className={cn(
        "rounded-lg border border-line bg-surface text-foreground shadow-sm",
        "p-4",
        className,
      )}
      {...props}
    >
      {title && (
        <div className="mb-3">
          <h3 className="font-serif text-base font-semibold leading-none tracking-tight text-foreground">
            {title}
          </h3>
          {subtitle && (
            <p className="mt-1 text-xs text-muted">{subtitle}</p>
          )}
        </div>
      )}
      {children}
    </div>
  );
}

export { Card };
