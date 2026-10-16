/** Joins class names, skipping empty ones. */
export function cx(...classes: (string | false | null | undefined)[]): string {
  return classes.filter(Boolean).join(" ");
}

const FOCUS = "focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent";

export const buttonClasses = {
  primary: cx(
    "rounded-lg bg-accent px-4 py-2 text-sm font-medium text-accent-fg hover:bg-accent-hover disabled:opacity-50",
    FOCUS,
  ),
  secondary: cx(
    "rounded-lg border border-line-strong px-3 py-1.5 text-sm hover:bg-surface-raised disabled:opacity-50",
    FOCUS,
  ),
  small: cx(
    "rounded-md bg-accent px-3 py-1 text-xs font-medium text-accent-fg hover:bg-accent-hover disabled:opacity-50",
    FOCUS,
  ),
};

export const inputClasses =
  "w-full rounded-lg border border-line bg-surface px-3 py-2 text-sm placeholder:text-fg-subtle focus:border-accent focus:outline-none";

export const labelClasses = "text-xs font-medium uppercase tracking-wider text-fg-muted";

export const cardClasses = "rounded-xl border border-line bg-surface/60 p-5";

/** A link inside text: same color as the text, underlined on hover. */
export const linkClasses = cx("rounded-sm hover:underline", FOCUS);

/** A link that stands out from the text around it. */
export const accentLinkClasses = cx("rounded-sm text-accent-text hover:underline", FOCUS);
