import type { ButtonHTMLAttributes } from "react";
import { buttonClasses, cx, type ButtonSize, type ButtonVariant } from "./styles";

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: ButtonSize;
  /** Shows a spinner and blocks the button while an action runs. */
  busy?: boolean;
}

export function Button({
  variant = "secondary",
  size = "md",
  busy = false,
  disabled,
  className,
  type = "button",
  children,
  ...rest
}: ButtonProps) {
  return (
    <button
      type={type}
      disabled={disabled || busy}
      aria-busy={busy || undefined}
      className={cx(buttonClasses(variant, size), className)}
      {...rest}
    >
      {busy && (
        <span
          aria-hidden="true"
          className="h-3 w-3 animate-spin rounded-full border-2 border-current border-t-transparent"
        />
      )}
      {children}
    </button>
  );
}
