import { useState } from "react";
import { useSearchParams } from "react-router";
import { api } from "../api/client";
import type { ModelScore, Overview } from "../api/types";
import { BarList } from "../components/charts/BarList";
import { ChartCard } from "../components/charts/ChartCard";
import { ColumnChart } from "../components/charts/ColumnChart";
import { DataTable } from "../components/charts/DataTable";
import { Empty, ErrorBox, Spinner } from "../components/Feedback";
import { messageOf, useAsync } from "../hooks/useAsync";
import { usePolling } from "../hooks/usePolling";
import { artistName, formatDay } from "../lib/format";
import { SOURCE_LABELS } from "../lib/labels";
import { buttonClasses, cardClasses, cx } from "../lib/styles";

const WINDOWS = [7, 30, 90];

export function DashboardPage() {
  const [search, setSearch] = useSearchParams();
  const days = WINDOWS.find((option) => option === Number(search.get("days"))) ?? 30;
  // the window goes into `refresh`, so the old charts stay while the new ones load
  const overview = useAsync(() => api.overview(days), "overview", days);

  return (
    <div className="space-y-6">
      <ModelQuality />
      <label className="flex items-center justify-end gap-3 text-sm text-fg-muted">
        Plays of the last
        <select
          value={days}
          onChange={(event) => setSearch({ days: event.target.value }, { replace: true })}
          className="rounded-lg border border-line bg-surface px-2 py-1.5 text-sm text-fg"
        >
          {WINDOWS.map((option) => (
            <option key={option} value={option}>
              {option} days
            </option>
          ))}
        </select>
        of the data (it ends in 2009)
      </label>
      {overview.error && <ErrorBox message={overview.error} onRetry={overview.reload} />}
      {!overview.data && !overview.error && <Spinner label="Counting plays..." />}
      {overview.data && (
        <div className={cx("space-y-6", overview.loading && "opacity-60")} aria-busy={overview.loading}>
          <Activity overview={overview.data} />
        </div>
      )}
    </div>
  );
}

function Activity({ overview }: { overview: Overview }) {
  if (overview.plays === 0 || overview.from === null || overview.to === null) {
    return <Empty title="No plays in this window">Load data with the etl command first.</Empty>;
  }
  const hours = overview.by_hour.map((plays, hour) => ({ label: String(hour), detail: `${hour}:00`, value: plays }));
  return (
    <>
      <ChartCard
        title="Plays per day"
        description={`${overview.plays.toLocaleString("en")} plays from ${formatDay(overview.from)} to ${formatDay(overview.to)}.`}
        table={{ columns: ["Day", "Plays"], rows: overview.by_day.map((item) => [item.day, item.plays]) }}
      >
        <ColumnChart
          label="Plays per day"
          unit="plays"
          points={overview.by_day.map((item) => ({ label: formatDay(item.day), detail: item.day, value: item.plays }))}
          maxLabels={8}
        />
      </ChartCard>
      <div className="grid gap-6 lg:grid-cols-2">
        <ChartCard
          title="Plays by hour"
          description="Hours as stored in the data, without time zones."
          table={{ columns: ["Hour", "Plays"], rows: hours.map((hour) => [hour.detail, hour.value]) }}
        >
          <ColumnChart label="Plays by hour of the day" unit="plays" points={hours} maxLabels={8} />
        </ChartCard>
        <ChartCard
          title="Top artists"
          table={{ columns: ["Artist", "Plays"], rows: overview.top_artists.map((artist) => [artistName(artist.name), artist.plays]) }}
        >
          <BarList
            label="Top artists by plays"
            items={overview.top_artists.map((artist) => ({
              key: artist.artist_index,
              label: artistName(artist.name),
              note: `${artist.listeners} listeners`,
              value: artist.plays,
            }))}
          />
        </ChartCard>
      </div>
    </>
  );
}

const MODEL_NAMES: Record<ModelScore["model"], string> = {
  ...SOURCE_LABELS,
  markov_2: "Next track (2 back)",
  hybrid: "Hybrid",
  hybrid_rules: "Hybrid + rules",
};

/** Every model alone, the hybrid and the hybrid with its rules, from `/model/evaluation`. */
function ModelQuality() {
  const evaluation = useAsync(() => api.evaluation(), "evaluation");
  const [error, setError] = useState<string | null>(null);
  const { reload } = evaluation;
  const running = evaluation.data?.running ?? false;

  usePolling(running, reload);

  async function start() {
    setError(null);
    try {
      await api.evaluate();
      reload();
    } catch (failure) {
      setError(messageOf(failure));
    }
  }

  const data = evaluation.data;
  const rows = data?.models ?? [];
  const best = rows.reduce((top, row, index) => (row.ndcg > rows[top].ndcg ? index : top), 0);
  return (
    <section aria-labelledby="quality-title" className={`${cardClasses} space-y-4`}>
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <h2 id="quality-title" className="text-lg font-semibold">
            Model quality
          </h2>
          <p className="mt-1 text-sm text-fg-muted">
            {data && rows.length > 0
              ? `Trained on the older 80% of plays, judged on the newer 20% (${data.users} listeners, top ${data.k}).`
              : "Each model alone and the hybrid, judged on plays they have not seen."}
          </p>
        </div>
        <button type="button" onClick={start} disabled={running} className={buttonClasses.secondary}>
          {running ? "Evaluating..." : rows.length > 0 ? "Evaluate again" : "Evaluate"}
        </button>
      </div>
      {evaluation.error && <ErrorBox message={evaluation.error} onRetry={reload} />}
      {error && <ErrorBox message={error} />}
      {data?.last_error && <ErrorBox message={`The last evaluation failed: ${data.last_error}`} />}
      {running && <Spinner label="Training on the older plays and ranking for every listener..." />}
      {rows.length === 0 && !running && <p className="text-sm text-fg-subtle">No evaluation yet.</p>}
      {rows.length > 0 && (
        <DataTable
          highlight={best}
          columns={["Model", "NDCG", "Hit rate", "Coverage", "Novelty, bits", "Artist diversity"]}
          rows={rows.map((row) => [
            MODEL_NAMES[row.model],
            row.ndcg.toFixed(4),
            row.hit_rate.toFixed(4),
            row.coverage.toFixed(4),
            row.novelty.toFixed(2),
            row.diversity?.toFixed(3) ?? "–",
          ])}
        />
      )}
    </section>
  );
}
