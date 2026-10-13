import type { Movement } from "../../api/types";

const STYLES: Record<Movement["kind"], { icon: string; classes: string }> = {
  up: { icon: "▲", classes: "text-accent-text" },
  down: { icon: "▼", classes: "text-danger-text" },
  new: { icon: "★", classes: "text-warning-text" },
  same: { icon: "=", classes: "text-fg-subtle" },
};

/** How a track moved: an arrow and a number, not only a color. */
export function MovementBadge({ movement }: { movement: Movement }) {
  const style = STYLES[movement.kind];
  const by = "by" in movement ? movement.by : null;
  const text = by === null ? (movement.kind === "new" ? "new in the list" : "same place") : `${movement.kind} ${by}`;
  return (
    <span title={text} className={`whitespace-nowrap text-xs ${style.classes}`}>
      <span aria-hidden="true">
        {style.icon}
        {by}
      </span>
      <span className="sr-only">{text}</span>
    </span>
  );
}
