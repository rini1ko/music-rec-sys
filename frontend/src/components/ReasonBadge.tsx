import type { Reason, Source } from "../api/types";

const STYLES: Record<Source, { label: string; classes: string }> = {
  trends: { label: "Trending", classes: "bg-amber-500/15 text-amber-200 ring-amber-500/30" },
  markov: { label: "Next track", classes: "bg-violet-500/15 text-violet-200 ring-violet-500/30" },
  co_listening: {
    label: "Played together",
    classes: "bg-emerald-500/15 text-emerald-200 ring-emerald-500/30",
  },
  cf: { label: "Similar listeners", classes: "bg-sky-500/15 text-sky-200 ring-sky-500/30" },
};

export function ReasonBadge({ reason }: { reason: Reason }) {
  const style = STYLES[reason.source];
  return (
    <span
      title={`${reason.text} (+${reason.contribution.toFixed(2)})`}
      className={`inline-flex items-center rounded-full px-2.5 py-0.5 text-xs ring-1 ring-inset ${style.classes}`}
    >
      {style.label}
    </span>
  );
}
