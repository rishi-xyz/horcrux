import * as React from "react";
import { cn } from "@/lib/utils";

export function Alert({ className, variant = "default", ...props }: React.HTMLAttributes<HTMLDivElement> & { variant?: "default" | "warn" | "danger" | "ok" | "info" }) {
  const styles = {
    default: "border-border bg-surface-2 text-text",
    warn: "border-warn/30 bg-warn/10 text-warn",
    danger: "border-danger/30 bg-danger/10 text-danger",
    ok: "border-ok/30 bg-ok/10 text-ok",
    info: "border-info/30 bg-info/10 text-info",
  }[variant];
  return <div role="alert" className={cn("relative w-full rounded-lg border px-4 py-3 text-sm", styles, className)} {...props} />;
}
export function AlertTitle({ className, ...props }: React.HTMLAttributes<HTMLHeadingElement>) {
  return <h5 className={cn("mb-1 font-medium leading-none tracking-tight", className)} {...props} />;
}
export function AlertDescription({ className, ...props }: React.HTMLAttributes<HTMLParagraphElement>) {
  return <div className={cn("text-sm opacity-90 [&_p]:leading-relaxed", className)} {...props} />;
}
