export interface TableData {
  columns: string[];
  rows: (string | number)[][];
}

/** A plain table; the first column names the row, the others are numbers aligned right. */
export function DataTable({ columns, rows, highlight }: TableData & { highlight?: number }) {
  return (
    <div className="overflow-x-auto">
      <table className="w-full text-left text-sm">
        <thead className="text-xs text-fg-muted">
          <tr>
            {columns.map((column) => (
              <th key={column} scope="col" className="py-1.5 pr-3 font-medium [&:not(:first-child)]:text-right">
                {column}
              </th>
            ))}
          </tr>
        </thead>
        <tbody className="divide-y divide-line">
          {rows.map(([name, ...values], index) => (
            <tr key={index} className={index === highlight ? "text-accent-text" : undefined}>
              <th scope="row" className="py-1.5 pr-3 font-normal">
                {name}
                {index === highlight && <span className="sr-only"> (best)</span>}
              </th>
              {values.map((value, column) => (
                <td key={column} className="py-1.5 pr-3 text-right tabular-nums">
                  {typeof value === "number" ? value.toLocaleString("en") : value}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
