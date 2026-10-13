import type { ReactNode } from "react";
import { buttonClasses } from "../lib/styles";

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
        <button type="button" onClick={onRetry} className={`${buttonClasses.secondary} mt-3`}>
          Try again
        </button>
      )}
    </div>
  );
}

/** Text for a list or a page that has nothing to show. */
export function Empty({ title, children }: { title: string; children?: ReactNode }) {
  return (
    <div className="rounded-xl border border-dashed border-line-strong px-6 py-10 text-center">
      <p className="font-medium">{title}</p>
      {children !== undefined && <div className="mt-2 text-sm text-fg-muted">{children}</div>}
    </div>
  );
}

/** One line under a form or a list after an action, read out by screen readers. */
export function StatusLine({ message }: { message: { ok: boolean; text: string } | null }) {
  if (message === null) return null;
  return (
    <p role="status" className={`text-sm ${message.ok ? "text-accent-text" : "text-danger-text"}`}>
      {message.text}
    </p>
  );
}
