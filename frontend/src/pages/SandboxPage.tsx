import { useState } from "react";
import { useSearchParams } from "react-router";
import { api } from "../api/client";
import type { Weights } from "../api/types";
import { ErrorBox, Spinner } from "../components/Feedback";
import { NoListener } from "../components/NoListener";
import { ComparisonView } from "../components/sandbox/ComparisonView";
import { WeightSliders } from "../components/sandbox/WeightSliders";
import { useAsync } from "../hooks/useAsync";
import { useDebounced } from "../hooks/useDebounced";
import { useUserParam } from "../hooks/useUserParam";
import { weightsFromSearch, weightsToSearch } from "../lib/weights";

export function SandboxPage() {
  const userId = useUserParam();
  const model = useAsync(() => api.model(), "model");

  if (typeof userId !== "number") {
    return <NoListener invalid={userId === null} purpose="compare the default weights with your own" />;
  }
  if (model.error) return <ErrorBox message={model.error} onRetry={model.reload} />;
  if (!model.data) return <Spinner label="Loading the default weights..." />;
  return <Sandbox key={userId} userId={userId} defaults={model.data.weights} />;
}

function Sandbox({ userId, defaults }: { userId: number; defaults: Weights }) {
  const [search, setSearch] = useSearchParams();
  // the router updates the URL in a transition, so sliders bound straight to it
  // lose fast key presses; the weights live in state and the URL follows them
  const [weights, setWeightsState] = useState(() => weightsFromSearch(search, defaults));
  const requestKey = useDebounced(JSON.stringify(weights), 300);
  // weights go into `refresh`, so the old lists stay on screen while a slider moves
  const comparison = useAsync(
    () => api.compare(userId, JSON.parse(requestKey) as Weights),
    `compare-${userId}`,
    requestKey,
  );

  function setWeights(next: Weights) {
    setWeightsState(next);
    setSearch((current) => weightsToSearch(current, next, defaults), { replace: true });
  }

  return (
    <div className="space-y-6">
      <WeightSliders weights={weights} defaults={defaults} onChange={setWeights} />
      {comparison.error && <ErrorBox message={comparison.error} onRetry={comparison.reload} />}
      {!comparison.data && !comparison.error && <Spinner label="Ranking tracks twice..." />}
      {comparison.data && (
        <div aria-busy={comparison.loading} className={comparison.loading ? "opacity-60" : undefined}>
          <ComparisonView comparison={comparison.data} />
        </div>
      )}
    </div>
  );
}
