import { Link, useLocation } from "react-router";
import { EmptyState } from "../components/ui/EmptyState";
import { buttonClasses } from "../components/ui/styles";
import { useDocumentTitle } from "../hooks/useDocumentTitle";
import { paths } from "../lib/paths";

export function NotFoundPage() {
  const { pathname } = useLocation();
  useDocumentTitle("Page not found");

  return (
    <EmptyState
      className="mt-12"
      title="Page not found"
      description={`There is no page at ${pathname}.`}
      action={
        <Link to={paths.listener("recommendations", null)} className={buttonClasses("primary")}>
          Go to recommendations
        </Link>
      }
    />
  );
}
