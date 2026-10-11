import { useId, useRef, type KeyboardEvent, type ReactNode } from "react";
import { cx } from "./styles";

export interface TabItem<T extends string> {
  id: T;
  label: string;
}

interface TabsProps<T extends string> {
  tabs: TabItem<T>[];
  value: T;
  onChange: (value: T) => void;
  /** What the tabs switch between, for screen readers. */
  label: string;
  /** Content of the selected tab. */
  children: ReactNode;
}

/**
 * Tabs by the WAI-ARIA pattern: only the selected tab is in the Tab order,
 * arrow keys, Home and End move between tabs.
 */
export function Tabs<T extends string>({ tabs, value, onChange, label, children }: TabsProps<T>) {
  const baseId = useId();
  const listRef = useRef<HTMLDivElement>(null);
  const tabId = (id: T) => `${baseId}-tab-${id}`;

  function select(index: number) {
    const next = tabs[(index + tabs.length) % tabs.length];
    onChange(next.id);
    listRef.current?.querySelector<HTMLButtonElement>(`#${CSS.escape(tabId(next.id))}`)?.focus();
  }

  function onKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    const current = tabs.findIndex((tab) => tab.id === value);
    const moves: Record<string, number> = {
      ArrowRight: current + 1,
      ArrowLeft: current - 1,
      Home: 0,
      End: tabs.length - 1,
    };
    const target = moves[event.key];
    if (target === undefined) return;
    event.preventDefault();
    select(target);
  }

  return (
    <div>
      <div
        ref={listRef}
        role="tablist"
        aria-label={label}
        onKeyDown={onKeyDown}
        className="inline-flex gap-1 rounded-lg border border-line bg-surface p-1"
      >
        {tabs.map((tab) => {
          const selected = tab.id === value;
          return (
            <button
              key={tab.id}
              id={tabId(tab.id)}
              type="button"
              role="tab"
              aria-selected={selected}
              aria-controls={`${baseId}-panel`}
              tabIndex={selected ? 0 : -1}
              onClick={() => onChange(tab.id)}
              className={cx(
                "rounded-md px-3 py-1.5 text-sm transition",
                "focus-visible:outline-2 focus-visible:outline-accent",
                selected ? "bg-surface-raised text-fg" : "text-fg-muted hover:text-fg",
              )}
            >
              {tab.label}
            </button>
          );
        })}
      </div>
      <div id={`${baseId}-panel`} role="tabpanel" aria-labelledby={tabId(value)} className="mt-4">
        {children}
      </div>
    </div>
  );
}
