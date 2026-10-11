import { useState } from "react";
import { useSearchParams } from "react-router";
import { api } from "../api/client";
import type { Recommendation } from "../api/types";
import { ErrorBox } from "../components/Feedback";
import { NoListener } from "../components/NoListener";
import { RecommendationCard } from "../components/RecommendationCard";
import { Button } from "../components/ui/Button";
import { Card } from "../components/ui/Card";
import { EmptyState } from "../components/ui/EmptyState";
import { SkeletonList } from "../components/ui/Skeleton";
import { cx, inputClasses, labelClasses } from "../components/ui/styles";
import { messageOf, useAsync } from "../hooks/useAsync";
import { useDocumentTitle } from "../hooks/useDocumentTitle";
import { useListenerEvents } from "../hooks/useListenerEvents";
import { useToast } from "../hooks/useToast";
import { useUserParam } from "../hooks/useUserParam";
import { trackTitle } from "../lib/format";

const LENGTHS = [10, 20, 30, 50] as const;
const DEFAULT_LENGTH = 20;

function lengthFrom(search: URLSearchParams): number {
  const value = Number(search.get("length"));
  return LENGTHS.find((option) => option === value) ?? DEFAULT_LENGTH;
}

export function RadioPage() {
  const userId = useUserParam();
  useDocumentTitle(typeof userId === "number" ? `Radio for user ${userId}` : "Radio");

  if (typeof userId !== "number") {
    return <NoListener invalid={userId === null} purpose="build a radio for them" />;
  }
  return <Radio key={userId} userId={userId} />;
}

function Radio({ userId }: { userId: number }) {
  const toast = useToast();
  const { notifyListened } = useListenerEvents();
  const [search, setSearch] = useSearchParams();
  const length = lengthFrom(search);
  const radio = useAsync(() => api.radio(userId, length), `radio-${userId}-${length}`);
  const [played, setPlayed] = useState<ReadonlySet<number>>(new Set());
  const [busyTrack, setBusyTrack] = useState<number | null>(null);

  async function listen(item: Recommendation) {
    setBusyTrack(item.track_index);
    try {
      await api.addEvent({ user: userId, track: item.track_index });
      setPlayed((current) => new Set(current).add(item.track_index));
      notifyListened();
      toast.show("success", `Played ${trackTitle(item.title)}`);
    } catch (error) {
      toast.show("error", messageOf(error));
    } finally {
      setBusyTrack(null);
    }
  }

  function rebuild() {
    setPlayed(new Set());
    radio.reload();
  }

  return (
    <div className="space-y-6">
      <Card
        title={`Radio for user ${userId}`}
        description="Each track is chosen as if the ones above it were already played. No track repeats, and one artist gets at most three tracks."
        actions={
          <Button size="sm" onClick={rebuild} busy={radio.loading && radio.data !== null}>
            Build again
          </Button>
        }
      >
        <div className="flex flex-wrap items-end gap-4">
          <div className="space-y-1.5">
            <label htmlFor="radio-length" className={labelClasses}>
              Tracks
            </label>
            <select
              id="radio-length"
              value={length}
              onChange={(event) => setSearch({ length: event.target.value }, { replace: true })}
              className={`${inputClasses} w-24`}
            >
              {LENGTHS.map((option) => (
                <option key={option} value={option}>
                  {option}
                </option>
              ))}
            </select>
          </div>
          {played.size > 0 && (
            <p role="status" className="text-sm text-fg-muted">
              {played.size === 1 ? "1 new play" : `${played.size} new plays`}. Build the radio again to
              start from {played.size === 1 ? "it" : "them"}.
            </p>
          )}
        </div>
      </Card>

      {radio.error && <ErrorBox message={radio.error} onRetry={radio.reload} />}
      {!radio.data && !radio.error && <SkeletonList rows={6} label="Building the radio..." />}
      {radio.data && radio.data.history_len === 0 && (
        <p role="note" className="rounded-lg border border-warning-line bg-warning-surface/40 p-3 text-sm text-warning-text">
          This listener has no plays yet. The radio starts from a popular track (in their country
          when it is known) and goes on from it.
        </p>
      )}
      {radio.data && radio.data.items.length === 0 && (
        <EmptyState title="Nothing to play" description="Every known track was already played." />
      )}
      {radio.data && radio.data.items.length > 0 && (
        <ol aria-label="Playlist" className={cx("space-y-3", radio.loading && "opacity-60")}>
          {radio.data.items.map((item, index) => {
            const done = played.has(item.track_index);
            return (
              <RecommendationCard
                key={item.track_index}
                rank={index + 1}
                item={item}
                maxScore={1}
                compact
                actions={
                  <Button
                    size="sm"
                    variant={done ? "ghost" : "primary"}
                    disabled={done || busyTrack !== null}
                    busy={busyTrack === item.track_index}
                    onClick={() => listen(item)}
                    aria-label={done ? `Played ${trackTitle(item.title)}` : `Listen to ${trackTitle(item.title)}`}
                  >
                    {done ? "✓ Played" : "Listen"}
                  </Button>
                }
              />
            );
          })}
        </ol>
      )}
    </div>
  );
}
