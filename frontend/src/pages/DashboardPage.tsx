import { Link, useSearchParams } from "react-router";
import { api } from "../api/client";
import type { Overview } from "../api/types";
import { AreaChart } from "../components/charts/AreaChart";
import { BarList } from "../components/charts/BarList";
import { ChartCard } from "../components/charts/ChartCard";
import { ColumnChart } from "../components/charts/ColumnChart";
import { ErrorBox } from "../components/Feedback";
import { EmptyState } from "../components/ui/EmptyState";
import { SkeletonList } from "../components/ui/Skeleton";
import { StatTile } from "../components/ui/StatTile";
import { cx, linkClasses } from "../components/ui/styles";
import { useAsync } from "../hooks/useAsync";
import { useDocumentTitle } from "../hooks/useDocumentTitle";
import { fillDays } from "../lib/chart";
import { WEEKDAYS, artistName, formatDate, formatDay, hourLabel, trackTitle } from "../lib/format";
import { paths } from "../lib/paths";

const WINDOWS = [7, 30, 90, 365] as const;
const DEFAULT_WINDOW = 30;
const TOP = 10;

function windowFrom(search: URLSearchParams): number {
  const days = Number(search.get("days"));
  return WINDOWS.find((option) => option === days) ?? DEFAULT_WINDOW;
}

export function DashboardPage() {
  useDocumentTitle("Dashboard");
  const [search, setSearch] = useSearchParams();
  const days = windowFrom(search);
  // the window goes into `refresh`, so the old charts stay while the new ones load
  const overview = useAsync(() => api.overview(days, TOP), "overview", days);

  function pick(next: number) {
    setSearch(next === DEFAULT_WINDOW ? {} : { days: String(next) }, { replace: true });
  }

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <h1 className="text-xl font-semibold">Dashboard</h1>
          <p className="text-sm text-fg-muted">
            The data is from 2009, so the window ends at the newest play, not today.
          </p>
        </div>
        <div role="group" aria-label="Window" className="flex gap-1 rounded-lg border border-line bg-surface p-1">
          {WINDOWS.map((option) => (
            <button
              key={option}
              type="button"
              aria-pressed={option === days}
              onClick={() => pick(option)}
              className={cx(
                "rounded-md px-3 py-1.5 text-sm transition focus-visible:outline-2 focus-visible:outline-accent",
                option === days ? "bg-surface-raised text-fg" : "text-fg-muted hover:text-fg",
              )}
            >
              {option === 365 ? "1 year" : `${option} days`}
            </button>
          ))}
        </div>
      </div>

      {overview.error && <ErrorBox message={overview.error} onRetry={overview.reload} />}
      {!overview.data && !overview.error && <SkeletonList rows={4} label="Counting plays..." rowClassName="h-40" />}
      {overview.data && (
        <div
          aria-busy={overview.loading}
          className={cx("space-y-6 transition-opacity", overview.loading && "opacity-60")}
        >
          <DashboardBody overview={overview.data} />
        </div>
      )}
    </div>
  );
}

function DashboardBody({ overview }: { overview: Overview }) {
  if (overview.plays === 0 || overview.from === null || overview.to === null) {
    return <EmptyState title="No plays in this window" description="Load data with the etl command first." />;
  }

  const days = fillDays(overview.by_day, overview.from, overview.to);
  const range = `${formatDate(overview.from)} – ${formatDate(overview.to)}`;

  return (
    <>
      <dl className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        <StatTile label="Plays" value={overview.plays} hint={range} />
        <StatTile label="Listeners" value={overview.listeners} />
        <StatTile label="Tracks played" value={overview.tracks} />
        <StatTile label="Artists played" value={overview.artists} />
      </dl>

      <ChartCard
        title="Plays per day"
        table={{ columns: ["Day", "Plays"], rows: days.map((item) => [item.day, item.plays]) }}
      >
        <AreaChart
          label="Plays per day"
          unit="plays"
          points={days.map((item) => ({ label: formatDay(item.day), detail: formatDate(item.day), value: item.plays }))}
        />
      </ChartCard>

      <div className="grid gap-6 lg:grid-cols-2">
        <ChartCard
          title="Plays by hour"
          description="Hours as stored in the data, without time zones."
          table={{ columns: ["Hour", "Plays"], rows: overview.by_hour.map((plays, hour) => [hourLabel(hour), plays]) }}
        >
          <ColumnChart
            label="Plays by hour of the day"
            unit="plays"
            labelEvery={3}
            points={overview.by_hour.map((plays, hour) => ({ label: String(hour), detail: hourLabel(hour), value: plays }))}
          />
        </ChartCard>
        <ChartCard
          title="Plays by weekday"
          table={{ columns: ["Weekday", "Plays"], rows: overview.by_weekday.map((plays, day) => [WEEKDAYS[day], plays]) }}
        >
          {/* Monday first, the API counts from Sunday */}
          <ColumnChart
            label="Plays by day of the week"
            unit="plays"
            points={[1, 2, 3, 4, 5, 6, 0].map((day) => ({ label: WEEKDAYS[day], value: overview.by_weekday[day] }))}
          />
        </ChartCard>
      </div>

      <div className="grid gap-6 lg:grid-cols-2">
        <ChartCard
          title="Top artists"
          table={{
            columns: ["Artist", "Plays", "Listeners"],
            rows: overview.top_artists.map((artist) => [artistName(artist.name), artist.plays, artist.listeners]),
          }}
        >
          <BarList
            label="Top artists by plays"
            unit="plays"
            items={overview.top_artists.map((artist) => ({
              key: artist.artist_index,
              title: artistName(artist.name),
              label: (
                <Link to={paths.artist(artist.artist_index)} className={linkClasses}>
                  {artistName(artist.name)}
                </Link>
              ),
              note: `${artist.listeners} listeners`,
              value: artist.plays,
            }))}
          />
        </ChartCard>
        <ChartCard
          title="Countries"
          table={{
            columns: ["Country", "Plays", "Listeners"],
            rows: overview.countries.map((item) => [item.country ?? "Not set", item.plays, item.listeners]),
          }}
        >
          <BarList
            label="Countries by plays"
            unit="plays"
            items={overview.countries.map((item) => ({
              key: item.country ?? "none",
              title: item.country ?? "Not set",
              label: item.country ?? <span className="text-fg-subtle">Not set</span>,
              note: `${item.listeners} listeners`,
              value: item.plays,
            }))}
          />
        </ChartCard>
      </div>

      <ChartCard
        title="Top tracks"
        table={{
          columns: ["Track", "Artist", "Plays"],
          rows: overview.top_tracks.map((track) => [trackTitle(track.title), artistName(track.artist), track.plays]),
        }}
      >
        <BarList
          label="Top tracks by plays"
          unit="plays"
          items={overview.top_tracks.map((track) => ({
            key: track.track_index,
            title: trackTitle(track.title),
            label: (
              <Link to={paths.track(track.track_index)} className={linkClasses}>
                {trackTitle(track.title)}
              </Link>
            ),
            note: (
              <Link to={paths.artist(track.artist_index)} className={linkClasses}>
                {artistName(track.artist)}
              </Link>
            ),
            value: track.plays,
          }))}
        />
      </ChartCard>
    </>
  );
}
