import { useEffect } from "react";

/** Calls `reload` every `everyMs` while `active` is true, e.g. while a training runs. */
export function usePolling(active: boolean, reload: () => void, everyMs = 1500) {
  useEffect(() => {
    if (!active) return;
    const timer = setInterval(reload, everyMs);
    return () => clearInterval(timer);
  }, [active, reload, everyMs]);
}
