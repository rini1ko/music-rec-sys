import { api } from "../api/client";
import { useAsync } from "../hooks/useAsync";
import { cx } from "./ui/styles";

export function StatusPill() {
  const health = useAsync(() => api.health(), "health");

  let label = "Checking the API...";
  let dot = "bg-fg-subtle";
  if (health.error) {
    label = "API offline";
    dot = "bg-danger";
  } else if (health.data) {
    label = `API online · ${health.data.events.toLocaleString()} events`;
    dot = "bg-accent";
  }

  return (
    <button
      type="button"
      onClick={health.reload}
      title="Check again"
      className="flex items-center gap-2 rounded-full border border-line px-3 py-1 text-xs text-fg-muted hover:border-line-strong"
    >
      <span aria-hidden="true" className={cx("h-2 w-2 rounded-full", dot)} />
      {label}
    </button>
  );
}
