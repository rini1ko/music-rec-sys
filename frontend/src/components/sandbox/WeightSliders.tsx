import type { WeightKey, Weights } from "../../api/types";
import { SOURCE_LABELS, WEIGHT_KEYS } from "../../lib/labels";
import { buttonClasses, cardClasses } from "../../lib/styles";
import { SLIDER_MAX, WEIGHT_STEP } from "../../lib/weights";

interface Props {
  weights: Weights;
  defaults: Weights;
  onChange: (weights: Weights) => void;
}

export function WeightSliders({ weights, defaults, onChange }: Props) {
  const total = WEIGHT_KEYS.reduce((sum, key) => sum + weights[key], 0);
  const changed = WEIGHT_KEYS.some((key) => weights[key] !== defaults[key]);

  // rounded to the step, so 0.1 + 0.2 does not end up as 0.30000000000000004 in the URL
  const set = (key: WeightKey, value: number) => onChange({ ...weights, [key]: Number(value.toFixed(2)) });

  return (
    <section aria-labelledby="weights-title" className={`${cardClasses} space-y-4`}>
      <div className="flex items-start justify-between gap-3">
        <div>
          <h2 id="weights-title" className="text-lg font-semibold">
            Model weights
          </h2>
          <p className="mt-1 text-sm text-fg-muted">
            Each model's scores are scaled to 0..1 and multiplied by its weight. Only the ratio matters.
          </p>
        </div>
        <button type="button" onClick={() => onChange(defaults)} disabled={!changed} className={buttonClasses.secondary}>
          Reset
        </button>
      </div>
      {WEIGHT_KEYS.map((key) => (
        <div key={key}>
          <div className="flex items-baseline justify-between gap-3 text-sm">
            <label htmlFor={`weight-${key}`} className="font-medium">
              {SOURCE_LABELS[key]}
            </label>
            <span className="text-xs tabular-nums text-fg-subtle">
              {weights[key].toFixed(2)} · {total > 0 ? Math.round((weights[key] / total) * 100) : 0}% · default{" "}
              {defaults[key]}
            </span>
          </div>
          <input
            id={`weight-${key}`}
            type="range"
            min={0}
            max={SLIDER_MAX}
            step={WEIGHT_STEP}
            value={Math.min(weights[key], SLIDER_MAX)}
            onChange={(event) => set(key, event.target.valueAsNumber)}
            className="mt-1 w-full cursor-pointer accent-accent"
          />
        </div>
      ))}
    </section>
  );
}
