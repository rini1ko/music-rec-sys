import { useState, type ReactNode } from "react";
import type { Recommendation } from "../../api/types";
import { compare, type ComparedItem, type RankedItem } from "../../lib/compare";
import { RecommendationCard } from "../RecommendationCard";
import { EmptyState } from "../ui/EmptyState";
import { Tabs, type TabItem } from "../ui/Tabs";
import { labelClasses } from "../ui/styles";
import { MovementBadge } from "./MovementBadge";

type View = "side" | "changes";

const VIEWS: TabItem<View>[] = [
  { id: "side", label: "Side by side" },
  { id: "changes", label: "What moved" },
];

interface Props {
  base: Recommendation[];
  custom: Recommendation[];
}

function maxScoreOf(items: Recommendation[]): number {
  return Math.max(0, ...items.map((item) => item.score));
}

function Column({ title, children }: { title: string; children: ReactNode }) {
  return (
    <div className="min-w-0">
      <h3 className={`${labelClasses} mb-3`}>{title}</h3>
      {children}
    </div>
  );
}

function TrackLine({ entry, detail }: { entry: RankedItem; detail: ReactNode }) {
  return (
    <li className="flex items-center justify-between gap-3 py-2 text-sm">
      <span className="min-w-0 truncate">
        {entry.item.title ?? "Unknown track"}
        <span className="text-fg-subtle"> · {entry.item.artist ?? "Unknown artist"}</span>
      </span>
      <span className="shrink-0 text-xs tabular-nums text-fg-muted">{detail}</span>
    </li>
  );
}

function Group<T extends RankedItem>({
  title,
  entries,
  render,
}: {
  title: string;
  entries: T[];
  render: (entry: T) => ReactNode;
}) {
  return (
    <div>
      <h3 className={`${labelClasses} mb-1`}>
        {title} <span className="text-fg-subtle">({entries.length})</span>
      </h3>
      {entries.length === 0 ? (
        <p className="py-2 text-sm text-fg-subtle">None.</p>
      ) : (
        <ul className="divide-y divide-line">
          {entries.map((entry) => (
            <TrackLine key={entry.item.track_index} entry={entry} detail={render(entry)} />
          ))}
        </ul>
      )}
    </div>
  );
}

function rankChange(entry: ComparedItem): string {
  return `#${entry.baseRank} → #${entry.rank}`;
}

export function ComparisonView({ base, custom }: Props) {
  const [view, setView] = useState<View>("side");
  const comparison = compare(base, custom);
  const baseMax = maxScoreOf(base);
  const customMax = maxScoreOf(custom);
  const identical = comparison.unchanged === custom.length && custom.length === base.length;

  return (
    <div className="space-y-4">
      <p role="status" className="text-sm text-fg-muted">
        {identical
          ? "Your weights give the same list in the same order."
          : `${comparison.risen.length} up, ${comparison.fallen.length} down, ${comparison.added.length} new, ${comparison.dropped.length} gone, ${comparison.unchanged} in the same place.`}
      </p>

      <Tabs tabs={VIEWS} value={view} onChange={setView} label="Comparison view">
        {view === "side" ? (
          <div className="grid gap-6 lg:grid-cols-2">
            <Column title="Default weights">
              {base.length === 0 ? (
                <EmptyState title="No tracks" />
              ) : (
                <ol className="space-y-3">
                  {base.map((item, index) => (
                    <RecommendationCard key={item.track_index} rank={index + 1} item={item} maxScore={baseMax} compact />
                  ))}
                </ol>
              )}
            </Column>
            <Column title="Your weights">
              {custom.length === 0 ? (
                <EmptyState title="No tracks" description="Turn at least one model on." />
              ) : (
                <ol className="space-y-3">
                  {comparison.items.map((entry) => (
                    <RecommendationCard
                      key={entry.item.track_index}
                      rank={entry.rank}
                      item={entry.item}
                      maxScore={customMax}
                      marker={<MovementBadge movement={entry.movement} />}
                      compact
                    />
                  ))}
                </ol>
              )}
            </Column>
          </div>
        ) : (
          <div className="grid gap-6 md:grid-cols-2">
            <Group title="Moved up" entries={comparison.risen} render={rankChange} />
            <Group title="Moved down" entries={comparison.fallen} render={rankChange} />
            <Group title="New in the list" entries={comparison.added} render={(entry) => `#${entry.rank}`} />
            <Group title="Gone from the list" entries={comparison.dropped} render={(entry) => `was #${entry.rank}`} />
          </div>
        )}
      </Tabs>
    </div>
  );
}
