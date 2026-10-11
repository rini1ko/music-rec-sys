import { useEffect, useState } from "react";
import { api } from "../api/client";
import { messageOf, useAsync } from "../hooks/useAsync";
import { ErrorBox, Spinner } from "./Feedback";

const POLL_MS = 1500;

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-xl border border-slate-800 bg-slate-900/60 p-4">
      <dt className="text-xs uppercase tracking-wider text-slate-400">{label}</dt>
      <dd className="mt-1 text-lg tabular-nums">{value}</dd>
    </div>
  );
}

export function ModelPanel() {
  const model = useAsync(() => api.model(), "model");
  const [actionError, setActionError] = useState<string | null>(null);
  const { reload } = model;
  const training = model.data?.training ?? false;

  // while a training runs, ask for its status until it ends
  useEffect(() => {
    if (!training) return;
    const timer = setInterval(reload, POLL_MS);
    return () => clearInterval(timer);
  }, [training, reload]);

  async function startTraining() {
    setActionError(null);
    try {
      await api.retrain();
      reload();
    } catch (error) {
      setActionError(messageOf(error));
    }
  }

  if (model.error) {
    return <ErrorBox message={model.error} onRetry={model.reload} />;
  }
  if (!model.data) {
    return <Spinner label="Loading the model status..." />;
  }

  const status = model.data;
  return (
    <section aria-label="Models" className="space-y-6">
      <div>
        <h2 className="text-lg font-semibold">Models</h2>
        <p className="mt-1 max-w-2xl text-sm text-slate-400">
          New plays change a listener's history at once. Trends, the co-listening graph and similar
          listeners learn from them only after the models are trained again. Requests are served by
          the old models until the new ones are ready.
        </p>
      </div>

      <dl className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        <Stat label="Trained" value={new Date(status.trained_at).toLocaleString()} />
        <Stat label="Training took" value={`${status.train_seconds.toFixed(1)} s`} />
        <Stat label="Events" value={status.events.toLocaleString()} />
        <Stat label="Sessions" value={status.sessions.toLocaleString()} />
      </dl>

      <div className="flex items-center gap-4">
        <button
          type="button"
          onClick={startTraining}
          disabled={status.training}
          className="rounded-lg bg-emerald-500 px-4 py-2 text-sm font-medium text-slate-950 hover:bg-emerald-400 disabled:opacity-50"
        >
          {status.training ? "Training..." : "Train again"}
        </button>
        {status.training && <Spinner label="Training on all stored events" />}
      </div>

      {actionError && <ErrorBox message={actionError} />}
      {status.last_error && (
        <ErrorBox message={`The last training failed: ${status.last_error}`} />
      )}
    </section>
  );
}