import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { readSnapshot } from './api';
import type { Snapshot } from './types';

export default function useSnapshot(
  mode: string,
  sensor: string | null,
  range: string,
  revision: number,
) {
  const source = useMemo(
    () => ({ mode, sensor, range, revision }),
    [mode, sensor, range, revision],
  );
  const active = useRef<{ source: typeof source } | null>(null);
  const pending = useRef(Promise.resolve());
  const [result, setResult] = useState<{
    source: typeof source;
    snapshot: Snapshot;
  }>();
  const [failure, setFailure] = useState<{
    source: typeof source;
    message: string;
  }>();
  const [loading, setLoading] = useState<typeof source | null>(null);

  const refresh = useCallback(() => {
    const session = active.current;
    // Serialize reads, including across source changes. A refresh after a
    // mutation must read again rather than reuse a snapshot taken before it.
    const task = pending.current.then(async () => {
      if (!session || session !== active.current || session.source !== source)
        return;
      setLoading(source);
      try {
        const snapshot = await readSnapshot(
          source.mode,
          source.sensor,
          source.range === 'all'
            ? null
            : Math.floor(Date.now() / 1000) - Number(source.range),
        );
        if (session === active.current) {
          setResult({ source, snapshot });
          setFailure(undefined);
        }
      } catch (e) {
        if (session === active.current)
          setFailure({ source, message: String(e) });
      } finally {
        if (session === active.current) setLoading(null);
      }
    });
    pending.current = task;
    return task;
  }, [source]);

  useEffect(() => {
    const session = { source };
    active.current = session;
    let timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      await refresh();
      if (active.current === session) timer = setTimeout(poll, 10000);
    };
    void poll();
    return () => {
      active.current = null;
      clearTimeout(timer);
    };
  }, [refresh, source]);

  return {
    snapshot: result?.source === source ? result.snapshot : null,
    error: failure?.source === source ? failure.message : '',
    loading: loading === source,
    refresh,
    dismissError: () => setFailure(undefined),
  };
}
