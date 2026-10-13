import { useState } from "react";
import { Link, NavLink, Outlet, useLocation } from "react-router";
import { matchListener, paths } from "../lib/paths";
import { cx } from "../lib/styles";

function navClasses(active: boolean): string {
  return cx(
    "rounded-md px-3 py-1.5 text-sm focus-visible:outline-2 focus-visible:outline-accent",
    active ? "bg-surface-raised text-fg" : "text-fg-muted hover:text-fg",
  );
}

/** Header with the sections and the page below it. */
export function AppShell() {
  const { pathname } = useLocation();
  const current = matchListener(pathname);
  // the last listener stays in the link, so going to Models and back keeps them
  const [lastUser, setLastUser] = useState<number | null>(null);
  if (current?.user != null && current.user !== lastUser) setLastUser(current.user);

  return (
    <div className="min-h-screen bg-canvas text-fg">
      <header className="border-b border-line">
        <div className="mx-auto flex max-w-6xl flex-wrap items-center justify-between gap-4 px-4 py-4 sm:px-6">
          <Link to={paths.home} className="text-lg font-semibold tracking-tight">
            Music recommender
          </Link>
          <nav aria-label="Sections" className="order-3 flex w-full gap-1 overflow-x-auto sm:order-none sm:w-auto">
            <Link
              to={paths.listener(current?.section ?? "recommendations", current?.user ?? lastUser)}
              aria-current={current ? "page" : undefined}
              className={navClasses(current !== null)}
            >
              Listeners
            </Link>
            <NavLink to={paths.dashboard} className={({ isActive }) => navClasses(isActive)}>
              Dashboard
            </NavLink>
            <NavLink to={paths.models} className={({ isActive }) => navClasses(isActive)}>
              Models
            </NavLink>
          </nav>
        </div>
      </header>
      <main className="mx-auto max-w-6xl px-4 py-8 sm:px-6">
        <Outlet />
      </main>
    </div>
  );
}
