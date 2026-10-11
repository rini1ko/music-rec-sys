import { useState } from "react";
import { Link, NavLink, Outlet, useLocation } from "react-router";
import { ErrorBoundary } from "../components/ErrorBoundary";
import { StatusPill } from "../components/StatusPill";
import { cx } from "../components/ui/styles";
import { matchListener, paths, type ListenerSection } from "../lib/paths";

/** Pages that do not belong to one listener. */
const GLOBAL_SECTIONS = [
  { to: paths.dashboard, label: "Dashboard" },
  { to: paths.artists, label: "Artists" },
  { to: paths.models, label: "Models" },
] as const;

function navClasses(active: boolean): string {
  return cx(
    "rounded-md px-3 py-1.5 text-sm transition focus-visible:outline-2 focus-visible:outline-accent",
    active ? "bg-surface-raised text-fg" : "text-fg-muted hover:text-fg",
  );
}

/** Header with the sections and the page below it. */
export function AppShell() {
  const { pathname } = useLocation();
  const current = matchListener(pathname);

  // the last listener and section stay in the link, so going to Models and back keeps them
  const [lastUser, setLastUser] = useState<number | null>(null);
  const [lastSection, setLastSection] = useState<ListenerSection>("recommendations");
  const currentUser = current?.user ?? null;
  if (currentUser !== null && currentUser !== lastUser) {
    setLastUser(currentUser);
  }
  if (current !== null && current.section !== lastSection) {
    setLastSection(current.section);
  }
  const linkUser = currentUser ?? lastUser;

  return (
    <div className="min-h-screen bg-canvas text-fg">
      <a
        href="#main"
        className="sr-only focus:not-sr-only focus:fixed focus:left-4 focus:top-4 focus:z-50 focus:rounded-md focus:bg-surface focus:px-3 focus:py-2"
      >
        Skip to content
      </a>
      <header className="border-b border-line">
        <div className="mx-auto flex max-w-6xl flex-wrap items-center justify-between gap-4 px-4 py-4 sm:px-6">
          <Link to={paths.home} className="text-lg font-semibold tracking-tight">
            Music recommender
          </Link>
          <nav aria-label="Sections" className="order-3 flex w-full gap-1 overflow-x-auto sm:order-none sm:w-auto">
            <Link
              to={paths.listener(lastSection, linkUser)}
              aria-current={current !== null ? "page" : undefined}
              className={navClasses(current !== null)}
            >
              Listeners
            </Link>
            {GLOBAL_SECTIONS.map((item) => (
              <NavLink key={item.to} to={item.to} className={({ isActive }) => navClasses(isActive)}>
                {item.label}
              </NavLink>
            ))}
          </nav>
          <StatusPill />
        </div>
      </header>

      <main id="main" className="mx-auto max-w-6xl px-4 py-8 sm:px-6">
        <ErrorBoundary resetKey={pathname}>
          <Outlet />
        </ErrorBoundary>
      </main>
    </div>
  );
}
