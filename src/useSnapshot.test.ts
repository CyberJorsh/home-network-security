// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest';
import { act, cleanup, renderHook, waitFor } from '@testing-library/react';
import useSnapshot from './useSnapshot';
import { readSnapshot } from './api';
import sample from '../public/sample.json';
import type { Snapshot } from './types';

vi.mock('./api', () => ({ readSnapshot: vi.fn() }));
const fixture = sample as Snapshot;
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((done, fail) => {
    resolve = done;
    reject = fail;
  });
  return { promise, resolve, reject };
}
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
  vi.useRealTimers();
});

it('skips obsolete queued sources and never publishes an old response', async () => {
  const first = deferred<Snapshot>();
  const latest = deferred<Snapshot>();
  vi.mocked(readSnapshot)
    .mockReturnValueOnce(first.promise)
    .mockReturnValueOnce(latest.promise);
  const { result, rerender } = renderHook(
    ({ sensor }) => useSnapshot('local', sensor, 'all', 0),
    { initialProps: { sensor: 'first' } },
  );
  await waitFor(() => expect(readSnapshot).toHaveBeenCalledTimes(1));
  rerender({ sensor: 'skipped' });
  rerender({ sensor: 'latest' });
  expect(result.current.snapshot).toBeNull();
  await act(async () => first.resolve(fixture));
  expect(result.current.snapshot).toBeNull();
  expect(readSnapshot).toHaveBeenCalledTimes(2);
  expect(readSnapshot).toHaveBeenLastCalledWith('local', 'latest', null);
  const next = { ...fixture, selectedSensor: 'latest' };
  await act(async () => latest.resolve(next));
  expect(result.current.snapshot).toBe(next);
});

it('performs a fresh read after a mutation even when an earlier read is pending', async () => {
  const before = deferred<Snapshot>();
  const after = { ...fixture, observationCount: 999 };
  vi.mocked(readSnapshot)
    .mockReturnValueOnce(before.promise)
    .mockResolvedValueOnce(after);
  const { result } = renderHook(() => useSnapshot('local', null, 'all', 0));
  await waitFor(() => expect(readSnapshot).toHaveBeenCalledTimes(1));
  let refreshed!: Promise<void>;
  act(() => {
    refreshed = result.current.refresh();
  });
  await act(async () => {
    before.resolve(fixture);
    await refreshed;
  });
  expect(readSnapshot).toHaveBeenCalledTimes(2);
  expect(result.current.snapshot).toBe(after);
});

it('retains same-source observations on failure and recovers on the next poll', async () => {
  vi.useFakeTimers();
  vi.mocked(readSnapshot)
    .mockResolvedValueOnce(fixture)
    .mockRejectedValueOnce(new Error('Collector unavailable'))
    .mockResolvedValueOnce({ ...fixture, observationCount: 999 });
  const { result } = renderHook(() => useSnapshot('local', null, 'all', 0));
  await act(async () => {
    await vi.advanceTimersByTimeAsync(0);
  });
  await act(async () => {
    await vi.advanceTimersByTimeAsync(10000);
  });
  expect(result.current.snapshot).toBe(fixture);
  expect(result.current.error).toContain('Collector unavailable');
  await act(async () => {
    await vi.advanceTimersByTimeAsync(10000);
  });
  expect(result.current.snapshot?.observationCount).toBe(999);
  expect(result.current.error).toBe('');
});

it('invalidates the loaded range and connection even when the sensor is unchanged', async () => {
  vi.useFakeTimers();
  vi.setSystemTime(new Date('2026-09-09T12:00:00Z'));
  vi.mocked(readSnapshot).mockResolvedValue(fixture);
  const { result, rerender } = renderHook(
    ({ range, revision }) => useSnapshot('local', null, range, revision),
    { initialProps: { range: 'all', revision: 0 } },
  );
  await act(async () => {
    await vi.advanceTimersByTimeAsync(0);
  });
  const filtered = deferred<Snapshot>();
  vi.mocked(readSnapshot).mockReturnValueOnce(filtered.promise);
  rerender({ range: '3600', revision: 0 });
  expect(result.current.snapshot).toBeNull();
  await act(async () => {
    await vi.advanceTimersByTimeAsync(0);
  });
  expect(readSnapshot).toHaveBeenLastCalledWith(
    'local',
    null,
    Date.now() / 1000 - 3600,
  );
  await act(async () => filtered.resolve(fixture));
  rerender({ range: '3600', revision: 1 });
  expect(result.current.snapshot).toBeNull();
  await act(async () => {
    await vi.advanceTimersByTimeAsync(0);
  });
  expect(readSnapshot).toHaveBeenCalledTimes(3);
});
