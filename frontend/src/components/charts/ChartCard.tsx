import type { ReactNode } from "react";
import { Card } from "../ui/Card";

export interface ChartTable {
  columns: string[];
  rows: (string | number)[][];
}

interface Props {
  title: string;
  description?: string;
  /** The same numbers as the chart, for keyboard and screen reader users. */
  table: ChartTable;
  children: ReactNode;
  className?: string;
}

/** A chart with its title and a table view of the same data under it. */
export function ChartCard({ title, description, table, children, className }: Props) {
  return (
    <Card title={title} description={description} className={className}>
      {children}
      <details className="mt-3 text-sm">
        <summary className="cursor-pointer text-xs text-fg-subtle hover:text-fg">Show as table</summary>
        <div className="mt-2 max-h-64 overflow-auto">
          <table className="w-full text-left text-xs">
            <thead className="text-fg-muted">
              <tr>
                {table.columns.map((column, index) => (
                  <th key={column} scope="col" className={index > 0 ? "py-1 text-right" : "py-1"}>
                    {column}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody className="divide-y divide-line">
              {table.rows.map((row, rowIndex) => (
                <tr key={rowIndex}>
                  {row.map((cell, index) => (
                    <td
                      key={index}
                      className={index > 0 ? "py-1 text-right tabular-nums" : "py-1"}
                    >
                      {typeof cell === "number" ? cell.toLocaleString("en") : cell}
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </details>
    </Card>
  );
}
