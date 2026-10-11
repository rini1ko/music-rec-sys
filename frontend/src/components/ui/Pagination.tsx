import { whole } from "../../lib/format";
import { Button } from "./Button";

interface Props {
  /** 1-based. */
  page: number;
  pageSize: number;
  total: number;
  onChange: (page: number) => void;
  /** Disables the buttons while a page loads. */
  busy?: boolean;
}

/** Previous and next buttons with "41-60 of 1,234". */
export function Pagination({ page, pageSize, total, onChange, busy = false }: Props) {
  const pages = Math.max(1, Math.ceil(total / pageSize));
  const first = total === 0 ? 0 : (page - 1) * pageSize + 1;
  const last = Math.min(total, page * pageSize);

  return (
    <nav aria-label="Pages" className="flex flex-wrap items-center justify-between gap-3 text-sm">
      <p className="text-fg-muted" aria-live="polite">
        {whole(first)}–{whole(last)} of {whole(total)}
      </p>
      <div className="flex items-center gap-2">
        <Button size="sm" onClick={() => onChange(1)} disabled={busy || page <= 1} aria-label="First page">
          «
        </Button>
        <Button size="sm" onClick={() => onChange(page - 1)} disabled={busy || page <= 1}>
          Previous
        </Button>
        <span className="px-1 text-xs tabular-nums text-fg-subtle">
          Page {whole(page)} of {whole(pages)}
        </span>
        <Button size="sm" onClick={() => onChange(page + 1)} disabled={busy || page >= pages}>
          Next
        </Button>
        <Button
          size="sm"
          onClick={() => onChange(pages)}
          disabled={busy || page >= pages}
          aria-label="Last page"
        >
          »
        </Button>
      </div>
    </nav>
  );
}
