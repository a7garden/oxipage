import * as React from "react";

import { cn } from "./cn";

/**
 * Page width wrapper. max-width matches --content-max-width (64rem).
 * Horizontal padding tracks --space-page-x (responsive clamp).
 */
function Container({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="container"
      className={cn(
        "mx-auto w-full max-w-[var(--content-max-width)] px-[var(--space-page-x)]",
        className,
      )}
      {...props}
    />
  );
}

export { Container };
