import { Link, useSearchParams } from "react-router";
import { api } from "../api/client";
import type { HistoryResponse } from "../api/types";
import { ErrorBox } from "../components/Feedback";
import { NoListener } from "../components/NoListener";
import { Card } from "../components/ui/Card";
import { EmptyState } from "../components/ui/EmptyState";
import { Pagination } from "../components/ui/Pagination";
import { SkeletonList } from "../components/ui/Skeleton";
import { cx, linkClasses } from "../components/ui/styles";
import { useAsync } from "../hooks/useAsync";
import { useDocumentTitle } from "../hooks/useDocumentTitle";
import { useUserParam } from "../hooks/useUserParam";
import { artistName, formatDateTime, trackTitle, whole } from "../lib/format";
import { paths } from "../lib/paths";

const PAGE_SIZE = 25;

function pageFrom(search: URLSearchParams): number {
  const page = Number(search.get("page"));
  return Number.isInteger(page) && page >= 1 ? page : 1;
}

export function HistoryPage() {
  const userId = useUserParam();
  useDocumentTitle(typeof userId === "number" ? `History of user ${userId}` : "History");

  if (typeof userId !== "number") {
    return <NoListener invalid={userId === null} purpose="see what they played" />;
  }
  return <History key={userId} userId={userId} />;
}

function History({ userId }: { userId: number }) {
  const [search, setSearch] = useSearchParams();
  const page = pageFrom(search);
  // the page goes into `refresh`, so the old rows stay while the next ones load
  const history = useAsync(
    () => api.history(userId, PAGE_SIZE, (page - 1) * PAGE_SIZE),
    `history-${userId}`,
    page,
  );

  function go(next: number) {
    // a new history entry per page, so Back returns to the previous page
    setSearch(next === 1 ? {} : { page: String(next) });
  }

  if (history.error) {
    return <ErrorBox message={history.error} onRetry={history.reload} />;
  }
  if (!history.data) {
    return <SkeletonList rows={8} label="Loading the history..." rowClassName="h-10" />;
  }

  const data = history.data;
  return (
    <Card
      title={`History of user ${userId}`}
      description={`${whole(data.total)} plays, newest first.`}
    >
      {data.total === 0 ? (
        <EmptyState
          title="No plays yet"
          description="Play a few tracks in the Player, they show up here at once."
          action={
            <Link to={paths.listener("player", userId)} className={linkClasses}>
              Open the Player
            </Link>
          }
        />
      ) : data.items.length === 0 ? (
        <EmptyState
          title="This page is past the end"
          action={
            <button type="button" onClick={() => go(1)} className={linkClasses}>
              Go to the first page
            </button>
          }
        />
      ) : (
        <div className="space-y-4">
          <HistoryTable data={data} busy={history.loading} />
          <Pagination
            page={page}
            pageSize={PAGE_SIZE}
            total={data.total}
            onChange={go}
            busy={history.loading}
          />
        </div>
      )}
    </Card>
  );
}

function HistoryTable({ data, busy }: { data: HistoryResponse; busy: boolean }) {
  return (
    <div className="overflow-x-auto">
      <table aria-busy={busy} className={cx("w-full text-left text-sm", busy && "opacity-60")}>
        <thead className="text-xs text-fg-muted">
          <tr>
            <th scope="col" className="w-12 py-2 pr-3 text-right font-medium">
              #
            </th>
            <th scope="col" className="py-2 pr-3 font-medium">
              Played
            </th>
            <th scope="col" className="py-2 pr-3 font-medium">
              Track
            </th>
            <th scope="col" className="py-2 font-medium">
              Artist
            </th>
          </tr>
        </thead>
        <tbody className="divide-y divide-line">
          {data.items.map((entry, index) => (
            <tr key={`${entry.timestamp}-${entry.track_index}-${index}`}>
              <td className="py-2 pr-3 text-right tabular-nums text-fg-subtle">
                {whole(data.offset + index + 1)}
              </td>
              <td className="whitespace-nowrap py-2 pr-3 tabular-nums text-fg-muted">
                {formatDateTime(entry.timestamp)}
              </td>
              <td className="max-w-64 truncate py-2 pr-3">
                <Link to={paths.track(entry.track_index)} className={linkClasses}>
                  {trackTitle(entry.title)}
                </Link>
              </td>
              <td className="max-w-48 truncate py-2">
                <Link to={paths.artist(entry.artist_index)} className={linkClasses}>
                  {artistName(entry.artist)}
                </Link>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
