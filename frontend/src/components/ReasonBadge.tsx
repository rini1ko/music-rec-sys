import type { Reason, Source } from "../api/types";
import { SOURCE_LABELS } from "../lib/labels";
import { cx } from "./ui/styles";

/** Each source keeps its own color, the label says the same in text. */
const COLORS: Record<Source, string> = {
  trends: "bg-amber-500/15 text-amber-200 ring-amber-500/30",
  markov: "bg-violet-500/15 text-violet-200 ring-violet-500/30",
  co_listening: "bg-emerald-500/15 text-emerald-200 ring-emerald-500/30",
  cf: "bg-sky-500/15 text-sky-200 ring-sky-500/30",
  artist: "bg-pink-500/15 text-pink-200 ring-pink-500/30",
  country: "bg-teal-500/15 text-teal-200 ring-teal-500/30",
  popular: "bg-slate-500/20 text-slate-200 ring-slate-500/30",
};

export function ReasonBadge({ reason }: { reason: Reason }) {
  return (
    <span
      title={`${reason.text} (+${reason.contribution.toFixed(2)})`}
      className={cx(
        "inline-flex items-center rounded-full px-2.5 py-0.5 text-xs ring-1 ring-inset",
        COLORS[reason.source],
      )}
    >
      {SOURCE_LABELS[reason.source]}
    </span>
  );
}
