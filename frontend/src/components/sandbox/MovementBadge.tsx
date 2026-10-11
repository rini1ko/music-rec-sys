import { describeMovement, type Movement } from "../../lib/compare";
import { cx } from "../ui/styles";

const STYLES: Record<Movement["kind"], { icon: string; classes: string }> = {
  up: { icon: "▲", classes: "text-accent-text" },
  down: { icon: "▼", classes: "text-danger-text" },
  new: { icon: "★", classes: "text-warning-text" },
  same: { icon: "=", classes: "text-fg-subtle" },
};

/** Shows how a track moved, with an arrow and a number, not only a color. */
export function MovementBadge({ movement }: { movement: Movement }) {
  const style = STYLES[movement.kind];
  const text = movement.kind === "up" || movement.kind === "down" ? String(movement.by) : movement.kind === "new" ? "new" : "";
  const description = describeMovement(movement);

  return (
    <span title={description} className={cx("whitespace-nowrap text-xs tabular-nums", style.classes)}>
      <span aria-hidden="true">
        {style.icon}
        {text}
      </span>
      <span className="sr-only">{description}</span>
    </span>
  );
}
