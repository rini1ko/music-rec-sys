import { ONBOARDING_PLAYS } from "../lib/labels";

interface Props {
  plays: number;
}

/** Guides a new listener through the first plays. Hidden once they are done. */
export function OnboardingProgress({ plays }: Props) {
  if (plays >= ONBOARDING_PLAYS) return null;
  const percent = Math.round((plays / ONBOARDING_PLAYS) * 100);

  return (
    <div className="rounded-lg border border-warning-line bg-warning-surface/40 p-4 text-sm">
      <p className="font-medium text-warning-text">
        Getting to know this listener: {plays} of {ONBOARDING_PLAYS} plays
      </p>
      <div
        role="progressbar"
        aria-label="Onboarding plays"
        aria-valuemin={0}
        aria-valuemax={ONBOARDING_PLAYS}
        aria-valuenow={plays}
        className="mt-2 h-1.5 overflow-hidden rounded-full bg-surface-raised"
      >
        <div className="h-full rounded-full bg-warning-text" style={{ width: `${percent}%` }} />
      </div>
      <p className="mt-2 text-fg-muted">
        {plays === 0
          ? "Search for a few tracks this listener likes and press Listen. Until the first play, the recommendations are popular tracks."
          : "Every play goes into the history at once, and the recommendations below are ranked again from it. The trained models learn from these plays only after the next training."}
      </p>
    </div>
  );
}
