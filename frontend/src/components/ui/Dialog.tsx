import { useEffect, useId, useRef, type ReactNode } from "react";

interface DialogProps {
  open: boolean;
  onClose: () => void;
  title: string;
  description?: ReactNode;
  children: ReactNode;
}

/**
 * A modal on top of the native <dialog>: the browser keeps focus inside it and
 * closes it on Escape. When it closes, focus goes back to the element that opened it.
 * The content is mounted only while the dialog is open, so forms start empty every time.
 * Mark the field that should get focus first with `data-autofocus`.
 */
export function Dialog({ open, onClose, title, description, children }: DialogProps) {
  const dialogRef = useRef<HTMLDialogElement>(null);
  const titleId = useId();
  const descriptionId = useId();

  useEffect(() => {
    const dialog = dialogRef.current;
    if (!open || dialog === null) return;

    const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    dialog.showModal();
    // showModal focuses the first focusable element, which is the close button;
    // React's autoFocus runs before the dialog is shown, so it is lost
    dialog.querySelector<HTMLElement>("[data-autofocus]")?.focus();
    return () => {
      dialog.close();
      opener?.focus();
    };
  }, [open]);

  return (
    <dialog
      ref={dialogRef}
      aria-labelledby={titleId}
      aria-describedby={description !== undefined ? descriptionId : undefined}
      onCancel={(event) => {
        // the parent owns the open state, so Escape only asks it to close
        event.preventDefault();
        onClose();
      }}
      onClose={() => {
        // some browsers close on a second Escape even after preventDefault
        if (open) onClose();
      }}
      onClick={(event) => {
        // a click on the backdrop lands on the <dialog> element itself
        if (event.target === event.currentTarget) onClose();
      }}
      className="m-auto w-[calc(100%-2rem)] max-w-md rounded-xl border border-line bg-surface p-0 text-fg shadow-2xl"
    >
      {open && (
        <div className="p-6">
          <div className="mb-5 flex items-start justify-between gap-4">
            <div>
              <h2 id={titleId} className="text-lg font-semibold">
                {title}
              </h2>
              {description !== undefined && (
                <p id={descriptionId} className="mt-1 text-sm text-fg-muted">
                  {description}
                </p>
              )}
            </div>
            <button
              type="button"
              onClick={onClose}
              aria-label="Close"
              className="rounded-md px-2 text-xl leading-none text-fg-muted hover:text-fg"
            >
              ×
            </button>
          </div>
          {children}
        </div>
      )}
    </dialog>
  );
}
