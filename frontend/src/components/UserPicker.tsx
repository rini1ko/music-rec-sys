import { api } from "../api/client";
import { useAsync } from "../hooks/useAsync";
import { buttonClasses, cx, labelClasses } from "../lib/styles";
import { ErrorBox, Spinner } from "./Feedback";

const PAGE_SIZE = 50;

interface Props {
  selected: number | null;
  onSelect: (userIndex: number) => void;
  onCreate: () => void;
  /** Change it to load the list again, e.g. after a play or a new listener. */
  refresh?: number;
}

export function UserPicker({ selected, onSelect, onCreate, refresh = 0 }: Props) {
  // `refresh` keeps the old list on screen while the new counts load
  const users = useAsync(() => api.users(PAGE_SIZE), "users", refresh);
  const listed = users.data?.items.some((user) => user.user_index === selected) ?? false;
  const rows = [
    ...(selected !== null && users.data && !listed
      ? [{ user_index: selected, country: null, plays: null }]
      : []),
    ...(users.data?.items ?? []),
  ];

  return (
    <section aria-label="Listeners" className="space-y-3">
      <div className="flex items-center justify-between gap-2">
        <h2 className={labelClasses}>Listeners</h2>
        <button type="button" onClick={onCreate} className={buttonClasses.secondary}>
          + New listener
        </button>
      </div>
      {users.error && <ErrorBox message={users.error} onRetry={users.reload} />}
      {!users.data && !users.error && <Spinner label="Loading listeners..." />}
      {users.data && (
        <>
          <ul className="max-h-[40vh] space-y-1 overflow-y-auto pr-1 md:max-h-[65vh]">
            {rows.map((user) => {
              const active = user.user_index === selected;
              return (
                <li key={user.user_index}>
                  <button
                    type="button"
                    onClick={() => onSelect(user.user_index)}
                    aria-pressed={active}
                    className={cx(
                      "flex w-full items-center justify-between rounded-lg px-3 py-2 text-left text-sm",
                      active ? "bg-accent/15 text-accent-text ring-1 ring-accent/40" : "hover:bg-surface",
                    )}
                  >
                    <span className="truncate">
                      User {user.user_index}
                      <span className="ml-2 text-fg-subtle">{user.country ?? ""}</span>
                    </span>
                    {user.plays !== null && (
                      <span className="text-xs text-fg-subtle">{user.plays.toLocaleString()}</span>
                    )}
                  </button>
                </li>
              );
            })}
          </ul>
          <p className="text-xs text-fg-subtle">Most active first, {users.data.total} in all</p>
        </>
      )}
    </section>
  );
}
