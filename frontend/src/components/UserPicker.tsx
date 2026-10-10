import { api } from "../api/client";
import { useAsync } from "../hooks/useAsync";
import { ErrorBox, Spinner } from "./Feedback";

interface Props {
  selected: number | null;
  onSelect: (userIndex: number) => void;
}

export function UserPicker({ selected, onSelect }: Props) {
  const users = useAsync(() => api.users(50), "users");

  return (
    <section aria-label="Listeners">
      <h2 className="mb-3 text-xs font-medium uppercase tracking-wider text-slate-400">
        Listeners
      </h2>

      {users.error && <ErrorBox message={users.error} onRetry={users.reload} />}
      {users.loading && !users.data && !users.error && <Spinner label="Loading listeners..." />}

      {users.data && (
        <>
          <ul className="max-h-[70vh] space-y-1 overflow-y-auto pr-1">
            {users.data.items.map((user) => {
              const active = selected === user.user_index;
              return (
                <li key={user.user_index}>
                  <button
                    type="button"
                    onClick={() => onSelect(user.user_index)}
                    aria-pressed={active}
                    className={`flex w-full items-center justify-between rounded-lg px-3 py-2 text-left text-sm transition ${
                      active
                        ? "bg-emerald-500/15 text-emerald-200 ring-1 ring-emerald-500/40"
                        : "text-slate-300 hover:bg-slate-900"
                    }`}
                  >
                    <span>
                      User {user.user_index}
                      <span className="ml-2 text-slate-500">{user.country ?? "unknown country"}</span>
                    </span>
                    <span className="text-xs text-slate-500">{user.plays.toLocaleString()}</span>
                  </button>
                </li>
              );
            })}
          </ul>
          <p className="mt-3 text-xs text-slate-500">
            Top {users.data.items.length} of {users.data.total} by number of plays
          </p>
        </>
      )}
    </section>
  );
}
