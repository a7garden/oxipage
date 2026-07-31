import { cva, type VariantProps } from "class-variance-authority";
import * as React from "react";

import { cn } from "./cn";

const badgeVariants = cva(
  "inline-flex items-center rounded-full border px-2.5 py-0.5 text-xs font-medium transition-colors focus:outline-none",
  {
    variants: {
      variant: {
        default: "border-transparent bg-primary text-primary-foreground",
        secondary: "border-line bg-surface text-muted",
        outline: "border-line text-foreground",
        accent: "border-transparent bg-star/15 text-star",
        destructive: "border-transparent bg-destructive/15 text-destructive",
        positive: "border-transparent bg-positive/15 text-positive",
        warning: "border-transparent bg-amber-500/15 text-amber-600",
      },
    },
    defaultVariants: { variant: "default" },
  },
);

function Badge({
  className,
  variant,
  ...props
}: React.ComponentProps<"span"> & VariantProps<typeof badgeVariants>) {
  return (
    <span
      data-slot="badge"
      className={cn(badgeVariants({ variant }), className)}
      {...props}
    />
  );
}

export { Badge, badgeVariants };
