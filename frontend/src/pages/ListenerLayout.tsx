import { useState } from "react";
import { Outlet, useLocation, useNavigate } from "react-router";
import type { UserProfile } from "../api/types";
import { NewListenerDialog } from "../components/NewListenerDialog";
import { UserPicker } from "../components/UserPicker";
import { useToast } from "../hooks/useToast";
import { matchListener, paths } from "../lib/paths";

/** The listener list on the left and the page of the picked listener on the right. */
export function ListenerLayout() {
  const navigate = useNavigate();
  const location = useLocation();
  const toast = useToast();
  const [dialogOpen, setDialogOpen] = useState(false);
  const [usersRefresh, setUsersRefresh] = useState(0);

  const current = matchListener(location.pathname);
  const section = current?.section ?? "recommendations";

  function select(user: number) {
    // the weights of the sandbox stay in the query string when another listener is picked
    navigate(paths.listener(section, user, location.search));
  }

  function created(user: UserProfile) {
    setDialogOpen(false);
    setUsersRefresh((value) => value + 1);
    toast.show("success", `Listener ${user.user_index} created. These are their cold start tracks.`);
    navigate(paths.listener("recommendations", user.user_index));
  }

  return (
    <div className="grid gap-8 md:grid-cols-[18rem_minmax(0,1fr)]">
      <UserPicker
        selected={current?.user ?? null}
        onSelect={select}
        onCreate={() => setDialogOpen(true)}
        refresh={usersRefresh}
      />
      <div className="min-w-0">
        <Outlet />
      </div>
      <NewListenerDialog open={dialogOpen} onClose={() => setDialogOpen(false)} onCreated={created} />
    </div>
  );
}
