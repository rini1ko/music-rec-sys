import { api } from "../api/client";
import type { UserSummary } from "../api/types";
import { useAsync } from "../hooks/useAsync";
import { ErrorBox } from "./Feedback";
import { Button } from "./ui/Button";
import { SkeletonList } from "./ui/Skeleton";
import { cx, labelClasses } from "./ui/styles";

const PAGE_SIZE = 50;

interface Props {
  selected: number | null;
  onSelect: (userIndex: number) => void;
  onCreate: () => void;
  /** Change it to load the list again, e.g. after a new listener was added. */
  refresh?: number;
}

function ListenerButton({
  user,
  active,
  onSelect,
}: {
  /** `country` and `plays` are undefined when the listener is not in the loaded list. */
  user: Pick<UserSummary, "user_index"> & { country?: string | null; plays?: number };
  active: boolean;
  onSelect: (userIndex: number) => void;
}) {
  return (
    <button
      type="button"
      onClick={() => onSelect(user.user_index)}
      aria-pressed={active}
      className={cx(
        "flex w-full items-center justify-between rounded-lg px-3 py-2 text-left text-sm transition",
        "focus-visible:outline-2 focus-visible:outline-accent",
        active ? "bg-accent/15 text-accent-text ring-1 ring-accent/40" : "text-fg hover:bg-surface",
      )}
    >
      <span className="min-w-0 truncate">
        User {user.user_index}
        {user.country !== undefined && (
          <span className="ml-2 text-fg-subtle">{user.country ?? "unknown country"}</span>
        )}
      </span>
      {user.plays !== undefined && (
        <span className="text-xs text-fg-subtle">{user.plays.toLocaleString()}</span>
      )}
    </button>
  );
}

export function UserPicker({ selected, onSelect, onCreate, refresh = 0 }: Props) {
  const users = useAsync(() => api.users(PAGE_SIZE), "users", refresh);
  const selectedListed = users.data?.items.some((user) => user.user_index === selected) ?? false;

  return (
    <section aria-label="Listeners" className="space-y-3">
      <div className="flex items-center justify-between gap-2">
        <h2 className={labelClasses}>Listeners</h2>
        <Button variant="secondary" size="sm" onClick={onCreate}>
          + New listener
        </Button>
      </div>

      {selected !== null && users.data && !selectedListed && (
        <div>
          <p className="mb-1 text-xs text-fg-subtle">Selected, not in the top {PAGE_SIZE}</p>
          <ListenerButton user={{ user_index: selected }} active onSelect={onSelect} />
        </div>
      )}

      {users.error && <ErrorBox message={users.error} onRetry={users.reload} />}
      {!users.data && !users.error && <SkeletonList rows={8} label="Loading listeners..." rowClassName="h-9" />}

      {users.data && (
        <>
          <ul className="max-h-[40vh] space-y-1 overflow-y-auto pr-1 md:max-h-[65vh]">
            {users.data.items.map((user) => (
              <li key={user.user_index}>
                <ListenerButton user={user} active={selected === user.user_index} onSelect={onSelect} />
              </li>
            ))}
          </ul>
          <p className="text-xs text-fg-subtle">
            Top {users.data.items.length} of {users.data.total.toLocaleString()} by number of plays
          </p>
        </>
      )}
    </section>
  );
}
