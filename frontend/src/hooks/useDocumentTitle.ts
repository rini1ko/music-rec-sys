import { useEffect } from "react";

const APP_NAME = "Music recommender";

/** Sets the browser tab title while the page is shown. */
export function useDocumentTitle(title: string) {
  useEffect(() => {
    document.title = `${title} · ${APP_NAME}`;
    return () => {
      document.title = APP_NAME;
    };
  }, [title]);
}
