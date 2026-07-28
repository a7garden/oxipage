import * as React from "react";
import { cn } from "./cn";

interface CardProps extends React.ComponentProps<"div"> {
  title?: string;
  subtitle?: string;
}

function Card({ className, title, subtitle, children, ...props }: CardProps) {
  return (
    <div
      className={cn(
        "rounded-lg border border-[#e8e4e0] bg-white p-4",
        className,
      )}
      {...props}
    >
      {title && (
        <div className="mb-3">
          <h3 className="text-sm font-semibold text-[#2d2934]">{title}</h3>
          {subtitle && (
            <p className="text-xs text-[#777] mt-0.5">{subtitle}</p>
          )}
        </div>
      )}
      {children}
    </div>
  );
}

export { Card };
