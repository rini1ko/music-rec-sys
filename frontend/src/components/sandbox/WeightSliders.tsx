import type { WeightKey, Weights } from "../../api/types";
import { MODEL_DESCRIPTIONS, SOURCE_LABELS, WEIGHT_KEYS } from "../../lib/labels";
import { MAX_WEIGHT, SLIDER_MAX, WEIGHT_STEP, roundWeight, sameWeights, weightShares } from "../../lib/weights";
import { Button } from "../ui/Button";
import { Card } from "../ui/Card";
import { cx } from "../ui/styles";

interface Props {
  weights: Weights;
  defaults: Weights;
  onChange: (weights: Weights) => void;
}

export function WeightSliders({ weights, defaults, onChange }: Props) {
  const shares = weightShares(weights);
  const allOff = WEIGHT_KEYS.every((key) => weights[key] === 0);

  function set(key: WeightKey, raw: number) {
    if (Number.isNaN(raw)) return;
    onChange({ ...weights, [key]: roundWeight(raw) });
  }

  return (
    <Card
      title="Model weights"
      description="Each model scores its own candidates, the scores are scaled to 0..1 and multiplied by the weight. Only the ratio between the weights matters."
      actions={
        <Button size="sm" onClick={() => onChange(defaults)} disabled={sameWeights(weights, defaults)}>
          Reset
        </Button>
      }
    >
      <div className="space-y-5">
        {WEIGHT_KEYS.map((key) => {
          const id = `weight-${key}`;
          const changed = weights[key] !== defaults[key];
          return (
            <div key={key}>
              <div className="flex items-baseline justify-between gap-3">
                <label htmlFor={id} className="text-sm font-medium">
                  {SOURCE_LABELS[key]}
                  {changed && <span className="ml-2 text-xs text-accent-text">changed</span>}
                </label>
                <span className="text-xs tabular-nums text-fg-subtle">
                  {Math.round(shares[key] * 100)}% · default {defaults[key]}
                </span>
              </div>
              <p id={`${id}-description`} className="text-xs text-fg-subtle">
                {MODEL_DESCRIPTIONS[key]}
              </p>
              <div className="mt-2 flex items-center gap-3">
                <input
                  id={id}
                  type="range"
                  min={0}
                  max={SLIDER_MAX}
                  step={WEIGHT_STEP}
                  value={Math.min(weights[key], SLIDER_MAX)}
                  onChange={(event) => set(key, event.target.valueAsNumber)}
                  aria-describedby={`${id}-description`}
                  className="h-2 flex-1 cursor-pointer accent-accent"
                />
                <input
                  type="number"
                  min={0}
                  max={MAX_WEIGHT}
                  step={WEIGHT_STEP}
                  value={weights[key]}
                  onChange={(event) => set(key, event.target.valueAsNumber)}
                  aria-label={`${SOURCE_LABELS[key]} weight`}
                  className={cx(
                    "w-20 rounded-md border border-line bg-surface px-2 py-1 text-right text-sm tabular-nums",
                    "focus:border-accent focus:outline-none",
                  )}
                />
              </div>
            </div>
          );
        })}

        {allOff && (
          <p role="alert" className="text-sm text-warning-text">
            Every model is off, so the list below has nothing to rank.
          </p>
        )}
      </div>
    </Card>
  );
}
