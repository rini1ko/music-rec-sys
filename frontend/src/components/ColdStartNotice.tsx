import { Link } from "react-router";
import type { Recommendation } from "../api/types";
import { paths } from "../lib/paths";
import { accentLinkClasses } from "../lib/styles";

/** Explains why a listener without plays gets popular tracks and what to do next. */
export function ColdStartNotice({ user, items, showPlayerLink }: { user: number; items: Recommendation[]; showPlayerLink: boolean }) {
  const local = items.some((item) => item.reasons.some((reason) => reason.source === "country"));
  return (
    <div role="note" className="mb-4 rounded-xl border border-warning-line bg-warning-surface/40 p-4 text-sm">
      <p className="font-medium text-warning-text">Cold start: this listener has no plays yet</p>
      <p className="mt-1 text-fg-muted">
        These are the most played tracks {local ? "in the listener's country" : "among all listeners"}. After
        the first plays the recommendations follow the listener's own history.
      </p>
      {showPlayerLink && (
        <Link to={paths.listener("player", user)} className={`${accentLinkClasses} mt-2 inline-block`}>
          Open the Player →
        </Link>
      )}
    </div>
  );
}
