import { useId, type ReactNode } from "react";
import { cx } from "./styles";

interface CardProps {
  title?: ReactNode;
  description?: ReactNode;
  /** Buttons or links shown on the right of the title. */
  actions?: ReactNode;
  className?: string;
  children?: ReactNode;
}

/** A bordered block. With a title it becomes a labelled section. */
export function Card({ title, description, actions, className, children }: CardProps) {
  const titleId = useId();
  const hasHeader = title !== undefined || actions !== undefined;

  return (
    <section
      aria-labelledby={title !== undefined ? titleId : undefined}
      className={cx("rounded-xl border border-line bg-surface/60 p-5", className)}
    >
      {hasHeader && (
        <div className="mb-4 flex flex-wrap items-start justify-between gap-3">
          <div className="min-w-0">
            {title !== undefined && (
              <h2 id={titleId} className="text-lg font-semibold">
                {title}
              </h2>
            )}
            {description !== undefined && (
              <p className="mt-1 text-sm text-fg-muted">{description}</p>
            )}
          </div>
          {actions !== undefined && <div className="flex shrink-0 gap-2">{actions}</div>}
        </div>
      )}
      {children}
    </section>
  );
}
