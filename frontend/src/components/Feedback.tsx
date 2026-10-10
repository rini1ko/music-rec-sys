export function Spinner({ label }: { label: string }) {
  return (
    <div className="flex items-center gap-3 py-6 text-sm text-slate-400" role="status">
      <span className="h-4 w-4 animate-spin rounded-full border-2 border-slate-600 border-t-emerald-400" />
      {label}
    </div>
  );
}

export function ErrorBox({ message, onRetry }: { message: string; onRetry?: () => void }) {
  return (
    <div
      className="rounded-lg border border-rose-900 bg-rose-950/40 p-4 text-sm text-rose-200"
      role="alert"
    >
      <p>{message}</p>
      {onRetry && (
        <button
          type="button"
          onClick={onRetry}
          className="mt-3 rounded-md border border-rose-700 px-3 py-1 text-rose-100 hover:bg-rose-900/50"
        >
          Try again
        </button>
      )}
    </div>
  );
}