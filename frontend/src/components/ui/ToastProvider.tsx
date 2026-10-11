import { useCallback, useMemo, useRef, useState, type ReactNode } from "react";
import { cx } from "./styles";
import { ToastContext, type ToastApi, type ToastKind } from "./toastContext";

const VISIBLE_MS = 4000;
const MAX_TOASTS = 4;

interface Toast {
  id: number;
  kind: ToastKind;
  text: string;
}

const KIND_STYLES: Record<ToastKind, { classes: string; icon: string; label: string }> = {
  success: { classes: "border-accent/40 text-accent-text", icon: "✓", label: "Done" },
  error: { classes: "border-danger-line text-danger-text", icon: "!", label: "Error" },
  info: { classes: "border-line-strong text-fg", icon: "i", label: "Note" },
};

/** Short messages in the corner that disappear by themselves. */
export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([]);
  const nextId = useRef(0);

  const dismiss = useCallback((id: number) => {
    setToasts((current) => current.filter((toast) => toast.id !== id));
  }, []);

  const show = useCallback(
    (kind: ToastKind, text: string) => {
      nextId.current += 1;
      const id = nextId.current;
      setToasts((current) => [...current, { id, kind, text }].slice(-MAX_TOASTS));
      setTimeout(() => dismiss(id), VISIBLE_MS);
    },
    [dismiss],
  );

  const api = useMemo<ToastApi>(() => ({ show }), [show]);

  return (
    <ToastContext.Provider value={api}>
      {children}
      <div
        aria-live="polite"
        className="pointer-events-none fixed inset-x-4 bottom-4 z-50 flex flex-col items-end gap-2 sm:left-auto sm:w-96"
      >
        {toasts.map((toast) => {
          const style = KIND_STYLES[toast.kind];
          return (
            <div
              key={toast.id}
              role={toast.kind === "error" ? "alert" : "status"}
              className={cx(
                "pointer-events-auto flex w-full items-start gap-3 rounded-lg border bg-surface px-4 py-3 text-sm shadow-xl",
                style.classes,
              )}
            >
              <span aria-hidden="true" className="font-bold">
                {style.icon}
              </span>
              <p className="min-w-0 flex-1 text-fg">
                <span className="sr-only">{style.label}: </span>
                {toast.text}
              </p>
              <button
                type="button"
                onClick={() => dismiss(toast.id)}
                aria-label="Dismiss"
                className="text-fg-muted hover:text-fg"
              >
                ×
              </button>
            </div>
          );
        })}
      </div>
    </ToastContext.Provider>
  );
}
