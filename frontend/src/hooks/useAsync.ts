import { useEffect, useState } from "react";

export interface AsyncState<T> {
  data: T | null;
  error: string | null;
  loading: boolean;
  reload: () => void;
}

interface Settled<T> {
  key: string;
  version: number;
  data: T | null;
  error: string | null;
}

export function messageOf(error: unknown): string {
  return error instanceof Error ? error.message : "Something went wrong";
}

/**
 * Runs `load` again whenever `key` changes or `reload` is called.
 * While the same key reloads, the old data stays visible.
 */
export function useAsync<T>(load: () => Promise<T>, key: string): AsyncState<T> {
  const [version, setVersion] = useState(0);
  const [settled, setSettled] = useState<Settled<T> | null>(null);

  useEffect(() => {
    let cancelled = false;
    load()
      .then((data) => {
        if (!cancelled) setSettled({ key, version, data, error: null });
      })
      .catch((error: unknown) => {
        if (!cancelled) setSettled({ key, version, data: null, error: messageOf(error) });
      });
    return () => {
      cancelled = true;
    };
    // `load` is a new closure on every render, `key` stands for what it depends on
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, version]);

  const current = settled !== null && settled.key === key ? settled : null;
  return {
    data: current?.data ?? null,
    error: current?.error ?? null,
    loading: settled === null || settled.key !== key || settled.version !== version,
    reload: () => setVersion((value) => value + 1),
  };
}