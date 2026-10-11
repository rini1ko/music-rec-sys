import { Link } from "react-router";
import type { Recommendation } from "../api/types";
import { ONBOARDING_PLAYS } from "../lib/labels";
import { paths } from "../lib/paths";
import { buttonClasses } from "./ui/styles";

interface Props {
  user: number;
  items: Recommendation[];
  /** Hides the link when the Player is already on the page. */
  showPlayerLink?: boolean;
}

/** Explains why a listener without plays gets popular tracks and what to do next. */
export function ColdStartNotice({ user, items, showPlayerLink = true }: Props) {
  const byCountry = items.some((item) => item.reasons.some((reason) => reason.source === "country"));

  return (
    <div
      role="note"
      className="mb-4 rounded-xl border border-warning-line bg-warning-surface/40 p-4 text-sm"
    >
      <p className="font-medium text-warning-text">Cold start: this listener has no plays yet</p>
      <p className="mt-1 text-fg-muted">
        {byCountry
          ? "These tracks are the most played in the listener's country."
          : "These tracks are the most played among all listeners, because the listener's country is not set or nobody from it is in the data."}{" "}
        Play {ONBOARDING_PLAYS} or more tracks, and the recommendations will start to follow this
        listener's own history.
      </p>
      {showPlayerLink && (
        <Link to={paths.listener("player", user)} className={`${buttonClasses("secondary", "sm")} mt-3`}>
          Open the Player
        </Link>
      )}
    </div>
  );
}
