import { cx } from "./styles";

/** A grey placeholder in the shape of content that is still loading. */
export function Skeleton({ className }: { className?: string }) {
  return <div aria-hidden="true" className={cx("animate-pulse rounded-md bg-surface-raised", className)} />;
}

interface SkeletonListProps {
  rows: number;
  /** Read by screen readers instead of the grey blocks. */
  label: string;
  rowClassName?: string;
}

export function SkeletonList({ rows, label, rowClassName = "h-20" }: SkeletonListProps) {
  return (
    <div role="status" aria-label={label} className="space-y-3">
      {Array.from({ length: rows }, (_, row) => (
        <Skeleton key={row} className={cx("w-full rounded-xl", rowClassName)} />
      ))}
    </div>
  );
}
