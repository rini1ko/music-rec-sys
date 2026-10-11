import type { ReactNode } from "react";
import { labelClasses } from "./styles";

interface FieldProps {
  /** Id of the input inside, so the label points at it. */
  htmlFor: string;
  label: string;
  hint?: string;
  /** Shown under the input instead of the hint. */
  error?: string | null;
  children: ReactNode;
}

/** A label, an input and a hint or an error under it. */
export function Field({ htmlFor, label, hint, error, children }: FieldProps) {
  return (
    <div className="space-y-1.5">
      <label htmlFor={htmlFor} className={labelClasses}>
        {label}
      </label>
      {children}
      {error ? (
        <p id={`${htmlFor}-error`} className="text-xs text-danger-text">
          {error}
        </p>
      ) : (
        hint !== undefined && (
          <p id={`${htmlFor}-hint`} className="text-xs text-fg-subtle">
            {hint}
          </p>
        )
      )}
    </div>
  );
}
