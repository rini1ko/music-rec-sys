import { useOutletContext } from "react-router";

/** What ListenerLayout gives the listener pages through the router outlet. */
export interface ListenerEvents {
  /** Call after a play is saved, so the listener list shows the new count. */
  notifyListened: () => void;
}

export function useListenerEvents(): ListenerEvents {
  return useOutletContext<ListenerEvents>();
}
