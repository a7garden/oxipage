import { cva, type VariantProps } from "class-variance-authority";
import * as React from "react";
import { cn } from "./cn";

const badgeVariants = cva(
  "inline-flex items-center rounded-full border px-2.5 py-0.5 text-xs font-medium",
  {
    variants: {
      variant: {
        active: "border-transparent bg-[oklch(60%_0.15_145)/0.15] text-[oklch(50%_0.15_145)]",
        inactive: "border-transparent bg-[#e8e4e0] text-[#888]",
        destructive: "border-transparent bg-[oklch(55%_0.19_25)/0.15] text-[oklch(50%_0.19_25)]",
        default: "border-[#e8e4e0] bg-white text-[#555]",
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
