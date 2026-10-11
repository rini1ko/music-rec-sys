import { useEffect, useState } from "react";
import { Link } from "react-router";
import { api } from "../api/client";
import { messageOf, useAsync } from "../hooks/useAsync";
import { useToast } from "../hooks/useToast";
import { SOURCE_LABELS, WEIGHT_KEYS } from "../lib/labels";
import { paths } from "../lib/paths";
import { weightShares } from "../lib/weights";
import { ErrorBox, Spinner } from "./Feedback";
import { Button } from "./ui/Button";
import { Card } from "./ui/Card";
import { SkeletonList } from "./ui/Skeleton";

const POLL_MS = 1500;

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-xl border border-line bg-surface/60 p-4">
      <dt className="text-xs uppercase tracking-wider text-fg-muted">{label}</dt>
      <dd className="mt-1 text-lg tabular-nums">{value}</dd>
    </div>
  );
}

export function ModelPanel() {
  const toast = useToast();
  const model = useAsync(() => api.model(), "model");
  const [starting, setStarting] = useState(false);
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
    setStarting(true);
    try {
      await api.retrain();
      toast.show("info", "Training started. The old models answer requests until it ends.");
      reload();
    } catch (error) {
      setActionError(messageOf(error));
    } finally {
      setStarting(false);
    }
  }

  if (model.error) {
    return <ErrorBox message={model.error} onRetry={model.reload} />;
  }
  if (!model.data) {
    return <SkeletonList rows={3} label="Loading the model status..." />;
  }

  const status = model.data;
  const shares = weightShares(status.weights);
  return (
    <div className="space-y-6">
      <Card
        title="Models"
        description="New plays change a listener's history at once. Trends, the co-listening graph and similar listeners learn from them only after the models are trained again. Requests are served by the old models until the new ones are ready."
      >
        <dl className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
          <Stat label="Trained" value={new Date(status.trained_at).toLocaleString()} />
          <Stat label="Training took" value={`${status.train_seconds.toFixed(1)} s`} />
          <Stat label="Events" value={status.events.toLocaleString()} />
          <Stat label="Sessions" value={status.sessions.toLocaleString()} />
        </dl>

        <div className="mt-6 flex flex-wrap items-center gap-4">
          <Button variant="primary" onClick={startTraining} busy={starting} disabled={status.training}>
            {status.training ? "Training..." : "Train again"}
          </Button>
          {status.training && <Spinner label="Training on all stored events" />}
        </div>

        {actionError && (
          <div className="mt-4">
            <ErrorBox message={actionError} />
          </div>
        )}
        {status.last_error && (
          <div className="mt-4">
            <ErrorBox message={`The last training failed: ${status.last_error}`} />
          </div>
        )}
      </Card>

      <Card
        title="Default weights"
        description="Every request uses these weights unless it sets its own."
        actions={
          <Link to={paths.listener("sandbox", null)} className="text-sm text-accent-text hover:underline">
            Try other weights
          </Link>
        }
      >
        <dl className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
          {WEIGHT_KEYS.map((key) => (
            <Stat
              key={key}
              label={SOURCE_LABELS[key]}
              value={`${status.weights[key]} (${Math.round(shares[key] * 100)}%)`}
            />
          ))}
        </dl>
      </Card>
    </div>
  );
}
