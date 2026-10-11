import type { ReactNode } from "react";
import { cx } from "./styles";

interface EmptyStateProps {
  title: string;
  description?: ReactNode;
  /** A button or a link that gets the user out of the empty state. */
  action?: ReactNode;
  className?: string;
}

export function EmptyState({ title, description, action, className }: EmptyStateProps) {
  return (
    <div
      className={cx(
        "flex flex-col items-center gap-2 rounded-xl border border-dashed border-line-strong px-6 py-10 text-center",
        className,
      )}
    >
      <p className="font-medium">{title}</p>
      {description !== undefined && <p className="max-w-md text-sm text-fg-muted">{description}</p>}
      {action !== undefined && <div className="mt-3">{action}</div>}
    </div>
  );
}
