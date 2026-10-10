import { api } from "../api/client";
import { useAsync } from "../hooks/useAsync";

export function StatusPill() {
  const health = useAsync(() => api.health(), "health");

  let label = "Checking the API...";
  let dot = "bg-slate-500";
  if (health.error) {
    label = "API offline";
    dot = "bg-rose-500";
  } else if (health.data) {
    label = `API online · ${health.data.events.toLocaleString()} events`;
    dot = "bg-emerald-400";
  }

  return (
    <button
      type="button"
      onClick={health.reload}
      title="Check again"
      className="flex items-center gap-2 rounded-full border border-slate-800 px-3 py-1 text-xs text-slate-300 hover:border-slate-600"
    >
      <span className={`h-2 w-2 rounded-full ${dot}`} />
      {label}
    </button>
  );
}