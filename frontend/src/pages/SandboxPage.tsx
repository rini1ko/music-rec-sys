import { useState } from "react";
import { useSearchParams } from "react-router";
import { api } from "../api/client";
import type { RecommendationsResponse, Weights } from "../api/types";
import { ErrorBox } from "../components/Feedback";
import { NoListener } from "../components/NoListener";
import { ComparisonView } from "../components/sandbox/ComparisonView";
import { WeightSliders } from "../components/sandbox/WeightSliders";
import { Button } from "../components/ui/Button";
import { SkeletonList } from "../components/ui/Skeleton";
import { inputClasses, labelClasses } from "../components/ui/styles";
import { messageOf, useAsync } from "../hooks/useAsync";
import { useDebounced } from "../hooks/useDebounced";
import { useDocumentTitle } from "../hooks/useDocumentTitle";
import { useToast } from "../hooks/useToast";
import { useUserParam } from "../hooks/useUserParam";
import {
  TOP_OPTIONS,
  topFromSearch,
  weightsFromKey,
  weightsFromSearch,
  weightsKey,
  weightsToSearch,
  type TopOption,
} from "../lib/weights";

const DEBOUNCE_MS = 300;

export function SandboxPage() {
  const userId = useUserParam();
  const model = useAsync(() => api.model(), "model");
  useDocumentTitle(typeof userId === "number" ? `Weights for user ${userId}` : "Weights");

  if (typeof userId !== "number") {
    return <NoListener invalid={userId === null} purpose="compare the default weights with your own" />;
  }
  if (model.error) {
    return <ErrorBox message={model.error} onRetry={model.reload} />;
  }
  if (!model.data) {
    return <SkeletonList rows={4} label="Loading the default weights..." />;
  }
  return <Sandbox key={userId} userId={userId} defaults={model.data.weights} />;
}

function Sandbox({ userId, defaults }: { userId: number; defaults: Weights }) {
  const toast = useToast();
  const [search, setSearch] = useSearchParams();
  // the router applies URL changes in a transition, so inputs bound straight to the URL
  // lose fast key presses; the weights live in state and the URL follows them
  const [weights, setWeightsState] = useState(() => weightsFromSearch(search, defaults));
  const top = topFromSearch(search);
  const requestKey = useDebounced(weightsKey(weights), DEBOUNCE_MS);

  const base = useAsync<RecommendationsResponse>(
    () => api.recommendations(userId, top),
    `sandbox-base-${userId}-${top}`,
  );
  // weights go into `refresh`, so the old list stays on screen while a slider moves
  const custom = useAsync<RecommendationsResponse>(
    () => api.recommendations(userId, top, weightsFromKey(requestKey)),
    `sandbox-custom-${userId}-${top}`,
    requestKey,
  );

  function setWeights(next: Weights) {
    setWeightsState(next);
    setSearch((current) => weightsToSearch(current, next, defaults), { replace: true });
  }

  function setTop(next: TopOption) {
    setSearch(
      (current) => {
        const params = new URLSearchParams(current);
        params.set("top", String(next));
        return params;
      },
      { replace: true },
    );
  }

  async function copyLink() {
    try {
      await navigator.clipboard.writeText(window.location.href);
      toast.show("success", "Link with these weights copied.");
    } catch (error) {
      toast.show("error", `Could not copy the link: ${messageOf(error)}`);
    }
  }

  const coldStart = base.data?.history_len === 0;

  return (
    <div className="space-y-6">
      <WeightSliders weights={weights} defaults={defaults} onChange={setWeights} />

      <div className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <h2 className="text-lg font-semibold">Default against your weights</h2>
          <p className="text-sm text-fg-muted">User {userId}. The same request, ranked twice.</p>
        </div>
        <div className="flex items-end gap-3">
          <div className="space-y-1.5">
            <label htmlFor="sandbox-top" className={labelClasses}>
              Tracks
            </label>
            <select
              id="sandbox-top"
              value={top}
              onChange={(event) => setTop(Number(event.target.value) as TopOption)}
              className={`${inputClasses} w-24`}
            >
              {TOP_OPTIONS.map((option) => (
                <option key={option} value={option}>
                  {option}
                </option>
              ))}
            </select>
          </div>
          <Button size="md" onClick={copyLink}>
            Copy link
          </Button>
        </div>
      </div>

      {coldStart && (
        <p role="note" className="rounded-lg border border-warning-line bg-warning-surface/40 p-3 text-sm text-warning-text">
          This listener has no plays yet. The cold start ranks popular tracks and ignores the weights, so both
          lists are the same.
        </p>
      )}

      {base.error && <ErrorBox message={base.error} onRetry={base.reload} />}
      {custom.error && <ErrorBox message={custom.error} onRetry={custom.reload} />}
      {!base.error && !custom.error && (!base.data || !custom.data) && (
        <SkeletonList rows={5} label="Ranking tracks twice..." />
      )}
      {base.data && custom.data && (
        <div aria-busy={custom.loading}>
          {custom.loading && (
            <p role="status" className="mb-2 text-xs text-fg-subtle">
              Updating...
            </p>
          )}
          <ComparisonView base={base.data.items} custom={custom.data.items} />
        </div>
      )}
    </div>
  );
}
