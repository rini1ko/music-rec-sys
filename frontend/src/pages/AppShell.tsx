import { useState } from "react";
import { Link, NavLink, Outlet, useLocation } from "react-router";
import { ErrorBoundary } from "../components/ErrorBoundary";
import { StatusPill } from "../components/StatusPill";
import { cx } from "../components/ui/styles";
import { LISTENER_SECTIONS, matchListener, paths } from "../lib/paths";

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

  // the last picked listener stays in the links, so going to Models and back keeps them
  const [lastUser, setLastUser] = useState<number | null>(null);
  const currentUser = current?.user ?? null;
  if (currentUser !== null && currentUser !== lastUser) {
    setLastUser(currentUser);
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
            {LISTENER_SECTIONS.map((item) => {
              const active = current?.section === item.id;
              return (
                <Link
                  key={item.id}
                  to={paths.listener(item.id, linkUser)}
                  aria-current={active ? "page" : undefined}
                  className={navClasses(active)}
                >
                  {item.label}
                </Link>
              );
            })}
            <NavLink to={paths.models} className={({ isActive }) => navClasses(isActive)}>
              Models
            </NavLink>
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
