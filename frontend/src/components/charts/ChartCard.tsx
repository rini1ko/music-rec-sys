import type { ReactNode } from "react";
import { cardClasses } from "../../lib/styles";
import { DataTable, type TableData } from "./DataTable";

interface Props {
  title: string;
  description?: string;
  /** The same numbers as the chart, for readers who cannot use the chart. */
  table: TableData;
  children: ReactNode;
}

/** A chart with its title and a table view of the same data under it. */
export function ChartCard({ title, description, table, children }: Props) {
  return (
    <section aria-label={title} className={cardClasses}>
      <h2 className="text-lg font-semibold">{title}</h2>
      {description && <p className="mt-1 text-sm text-fg-muted">{description}</p>}
      <div className="mt-4">{children}</div>
      <details className="mt-3 text-xs">
        <summary className="cursor-pointer text-fg-subtle hover:text-fg">Show as table</summary>
        <div className="mt-2 max-h-64 overflow-auto">
          <DataTable {...table} />
        </div>
      </details>
    </section>
  );
}
