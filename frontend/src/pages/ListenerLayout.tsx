import { useCallback, useMemo, useState } from "react";
import { Link, Outlet, useLocation, useNavigate } from "react-router";
import type { UserProfile } from "../api/types";
import { NewListenerDialog } from "../components/NewListenerDialog";
import { UserPicker } from "../components/UserPicker";
import type { ListenerEvents } from "../hooks/useListenerEvents";
import { LISTENER_SECTIONS, matchListener, paths } from "../lib/paths";
import { cx } from "../lib/styles";

/** The listener list on the left, the sections of the picked listener on the right. */
export function ListenerLayout() {
  const navigate = useNavigate();
  const location = useLocation();
  const [dialogOpen, setDialogOpen] = useState(false);
  const [usersRefresh, setUsersRefresh] = useState(0);
  const current = matchListener(location.pathname);
  const section = current?.section ?? "recommendations";
  const user = current?.user ?? null;

  // pages call it after a play, so the play counts in the list change at once
  const notifyListened = useCallback(() => setUsersRefresh((value) => value + 1), []);
  const events = useMemo<ListenerEvents>(() => ({ notifyListened }), [notifyListened]);

  function created(created: UserProfile) {
    setDialogOpen(false);
    setUsersRefresh((value) => value + 1);
    navigate(paths.listener("recommendations", created.user_index));
  }

  return (
    <div className="grid gap-8 md:grid-cols-[18rem_minmax(0,1fr)]">
      <UserPicker
        selected={user}
        // the sandbox weights stay in the query string when another listener is picked
        onSelect={(next) => navigate(paths.listener(section, next, location.search))}
        onCreate={() => setDialogOpen(true)}
        refresh={usersRefresh}
      />
      <div className="min-w-0 space-y-6">
        <nav aria-label="Listener sections" className="flex gap-1 overflow-x-auto shadow-[inset_0_-1px_0_var(--line)]">
          {LISTENER_SECTIONS.map((item) => (
            <Link
              key={item.id}
              to={paths.listener(item.id, user)}
              aria-current={item.id === section ? "page" : undefined}
              className={cx(
                "whitespace-nowrap border-b-2 px-3 py-2 text-sm",
                item.id === section ? "border-accent" : "border-transparent text-fg-muted hover:text-fg",
              )}
            >
              {item.label}
            </Link>
          ))}
        </nav>
        <Outlet context={events} />
      </div>
      {dialogOpen && <NewListenerDialog onClose={() => setDialogOpen(false)} onCreated={created} />}
    </div>
  );
}
