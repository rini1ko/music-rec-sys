import { Button } from "./ui/Button";

export function Spinner({ label }: { label: string }) {
  return (
    <div className="flex items-center gap-3 py-6 text-sm text-fg-muted" role="status">
      <span
        aria-hidden="true"
        className="h-4 w-4 animate-spin rounded-full border-2 border-line-strong border-t-accent"
      />
      {label}
    </div>
  );
}

export function ErrorBox({ message, onRetry }: { message: string; onRetry?: () => void }) {
  return (
    <div
      className="rounded-lg border border-danger-line bg-danger-surface/40 p-4 text-sm text-danger-text"
      role="alert"
    >
      <p>{message}</p>
      {onRetry && (
        <Button variant="danger" size="sm" onClick={onRetry} className="mt-3">
          Try again
        </Button>
      )}
    </div>
  );
}
