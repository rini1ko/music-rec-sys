/** Joins class names, skipping empty ones. */
export function cx(...classes: (string | false | null | undefined)[]): string {
  return classes.filter(Boolean).join(" ");
}

export type ButtonVariant = "primary" | "secondary" | "ghost" | "danger";
export type ButtonSize = "sm" | "md";

const VARIANTS: Record<ButtonVariant, string> = {
  primary: "bg-accent text-accent-fg hover:bg-accent-hover",
  secondary: "border border-line-strong text-fg hover:bg-surface-raised",
  ghost: "text-fg-muted hover:bg-surface-raised hover:text-fg",
  danger: "border border-danger-line text-danger-text hover:bg-danger-surface",
};

const SIZES: Record<ButtonSize, string> = {
  sm: "px-3 py-1 text-xs",
  md: "px-4 py-2 text-sm",
};

/** Classes of a button. Links that look like buttons use them too. */
export function buttonClasses(variant: ButtonVariant = "secondary", size: ButtonSize = "md"): string {
  return cx(
    "inline-flex items-center justify-center gap-2 rounded-lg font-medium transition",
    "focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent",
    "disabled:cursor-not-allowed disabled:opacity-50",
    VARIANTS[variant],
    SIZES[size],
  );
}

export const inputClasses = cx(
  "w-full rounded-lg border border-line bg-surface px-3 py-2 text-sm text-fg",
  "placeholder:text-fg-subtle focus:border-accent focus:outline-none",
  "aria-invalid:border-danger",
);

export const labelClasses = "text-xs font-medium uppercase tracking-wider text-fg-muted";

/** A link inside text: same color as the text, underlined on hover. */
export const linkClasses =
  "rounded-sm hover:underline focus-visible:outline-2 focus-visible:outline-accent";

/** A link that should stand out from the text around it. */
export const accentLinkClasses =
  "rounded-sm text-accent-text hover:underline focus-visible:outline-2 focus-visible:outline-accent";
