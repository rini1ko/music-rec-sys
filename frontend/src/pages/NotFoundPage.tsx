import { Link, useLocation } from "react-router";
import { Empty } from "../components/Feedback";
import { paths } from "../lib/paths";
import { accentLinkClasses } from "../lib/styles";

export function NotFoundPage() {
  const { pathname } = useLocation();

  return (
    <Empty title="Page not found">
      There is no page at {pathname}.{" "}
      <Link to={paths.listener("recommendations", null)} className={accentLinkClasses}>
        Go to recommendations
      </Link>
    </Empty>
  );
}
