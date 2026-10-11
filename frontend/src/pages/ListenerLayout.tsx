import { useCallback, useMemo, useState } from "react";
import { Link, Outlet, useLocation, useNavigate } from "react-router";
import type { UserProfile } from "../api/types";
import { NewListenerDialog } from "../components/NewListenerDialog";
import { UserPicker } from "../components/UserPicker";
import { cx } from "../components/ui/styles";
import type { ListenerEvents } from "../hooks/useListenerEvents";
import { useToast } from "../hooks/useToast";
import { LISTENER_SECTIONS, matchListener, paths } from "../lib/paths";

/** The listener list on the left, the sections of the picked listener on the right. */
export function ListenerLayout() {
  const navigate = useNavigate();
  const location = useLocation();
  const toast = useToast();
  const [dialogOpen, setDialogOpen] = useState(false);
  const [usersRefresh, setUsersRefresh] = useState(0);

  const current = matchListener(location.pathname);
  const section = current?.section ?? "recommendations";
  const user = current?.user ?? null;

  const notifyListened = useCallback(() => setUsersRefresh((value) => value + 1), []);
  const events = useMemo<ListenerEvents>(() => ({ notifyListened }), [notifyListened]);

  function select(next: number) {
    // query parameters (weights, radio length) stay when another listener is picked
    navigate(paths.listener(section, next, location.search));
  }

  function created(created: UserProfile) {
    setDialogOpen(false);
    setUsersRefresh((value) => value + 1);
    toast.show("success", `Listener ${created.user_index} created. These are their cold start tracks.`);
    navigate(paths.listener("recommendations", created.user_index));
  }

  return (
    <div className="grid gap-8 md:grid-cols-[18rem_minmax(0,1fr)]">
      <UserPicker
        selected={user}
        onSelect={select}
        onCreate={() => setDialogOpen(true)}
        refresh={usersRefresh}
      />
      <div className="min-w-0 space-y-6">
        {/* the bottom line is a shadow: a border plus a -1px margin would overflow and show a scrollbar */}
        <nav
          aria-label="Listener sections"
          className="flex gap-1 overflow-x-auto shadow-[inset_0_-1px_0_var(--line)]"
        >
          {LISTENER_SECTIONS.map((item) => {
            const active = item.id === section;
            return (
              <Link
                key={item.id}
                to={paths.listener(item.id, user)}
                aria-current={active ? "page" : undefined}
                className={cx(
                  "whitespace-nowrap border-b-2 px-3 py-2 text-sm transition",
                  "focus-visible:outline-2 focus-visible:outline-accent",
                  active
                    ? "border-accent text-fg"
                    : "border-transparent text-fg-muted hover:text-fg",
                )}
              >
                {item.label}
              </Link>
            );
          })}
        </nav>
        <Outlet context={events} />
      </div>
      <NewListenerDialog open={dialogOpen} onClose={() => setDialogOpen(false)} onCreated={created} />
    </div>
  );
}
