import { useCallback, useEffect, useState } from "react";

export interface AsyncState<T> {
  data: T | null;
  error: string | null;
  loading: boolean;
  reload: () => void;
}

interface Settled<T> {
  key: string;
  stamp: string;
  data: T | null;
  error: string | null;
}

export function messageOf(error: unknown): string {
  return error instanceof Error ? error.message : "Something went wrong";
}

/**
 * Loads data again whenever `key` changes, `refresh` changes or `reload` is called.
 * When only `refresh` or `reload` triggers it, the old data stays visible meanwhile,
 * so use `refresh` for changes that should not blank the screen (e.g. slider values).
 */
export function useAsync<T>(
  load: () => Promise<T>,
  key: string,
  refresh: number | string = 0,
): AsyncState<T> {
  const [version, setVersion] = useState(0);
  const [settled, setSettled] = useState<Settled<T> | null>(null);
  const stamp = `${refresh}:${version}`;
  const reload = useCallback(() => setVersion((value) => value + 1), []);

  useEffect(() => {
    let cancelled = false;
    load()
      .then((data) => {
        if (!cancelled) setSettled({ key, stamp, data, error: null });
      })
      .catch((error: unknown) => {
        if (!cancelled) setSettled({ key, stamp, data: null, error: messageOf(error) });
      });
    return () => {
      cancelled = true;
    };
    // `load` is a new closure on every render, `key` and `stamp` stand for what it depends on
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, stamp]);

  const current = settled !== null && settled.key === key ? settled : null;
  return {
    data: current?.data ?? null,
    error: current?.error ?? null,
    loading: settled === null || settled.key !== key || settled.stamp !== stamp,
    reload,
  };
}