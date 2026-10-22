import { useState } from "react";
import { api } from "../api/client";
import { messageOf, useAsync } from "../hooks/useAsync";
import { usePolling } from "../hooks/usePolling";
import { SOURCE_LABELS, WEIGHT_KEYS } from "../lib/labels";
import { buttonClasses, cardClasses } from "../lib/styles";
import { ErrorBox, Spinner } from "./Feedback";

export function ModelPanel() {
  const model = useAsync(() => api.model(), "model");
  const [actionError, setActionError] = useState<string | null>(null);
  const { reload } = model;
  const training = model.data?.training ?? false;

  // while a training runs, ask for its status until it ends
  usePolling(training, reload);

  async function startTraining() {
    setActionError(null);
    try {
      await api.retrain();
      reload();
    } catch (error) {
      setActionError(messageOf(error));
    }
  }

  if (model.error) return <ErrorBox message={model.error} onRetry={model.reload} />;
  if (!model.data) return <Spinner label="Loading the model status..." />;

  const status = model.data;
  return (
    <section aria-labelledby="models-title" className={`${cardClasses} space-y-4`}>
      <h2 id="models-title" className="text-lg font-semibold">
        Models
      </h2>
      <p className="max-w-2xl text-sm text-fg-muted">
        New plays change a listener's history and the next-track and trend models at once. The
        co-listening graph, similar listeners and the hidden taste model learn from them only after
        the models are trained again; the old models answer until the new ones are ready.
      </p>
      <p className="text-sm">
        Trained {new Date(status.trained_at).toLocaleString()} in {status.train_seconds.toFixed(1)} s on{" "}
        {status.events.toLocaleString()} plays and {status.sessions.toLocaleString()} sessions.
      </p>
      <p className="text-sm text-fg-muted">
        Default weights: {WEIGHT_KEYS.map((key) => `${SOURCE_LABELS[key]} ${status.weights[key]}`).join(" · ")}
      </p>
      <div className="flex flex-wrap items-center gap-4">
        <button type="button" onClick={startTraining} disabled={status.training} className={buttonClasses.primary}>
          {status.training ? "Training..." : "Train again"}
        </button>
        {status.training && <Spinner label="Training on all stored events" />}
      </div>
      {actionError && <ErrorBox message={actionError} />}
      {status.last_error && <ErrorBox message={`The last training failed: ${status.last_error}`} />}
    </section>
  );
}
