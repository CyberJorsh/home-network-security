// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest';
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
} from '@testing-library/react';
import HostCollection from './HostCollection';
import { command } from '../api';

vi.mock('../api', () => ({ native: true, command: vi.fn() }));

const host = {
  interfaces: [{ id: 'fixture0', label: 'Fixture interface' }],
  addresses: [],
  suggestedCidrs: ['10.42.0.0/24'],
  captureError: null,
  captureRemedy: null,
  discoveryAvailable: true,
  platform: 'macos',
};
const idle = {
  running: false,
  kind: '',
  count: 0,
  sensorId: null,
  error: null,
};
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}
afterEach(() => {
  cleanup();
  vi.useRealTimers();
  vi.mocked(command).mockReset();
});

it('keeps a newly started capture when an older status request finishes', async () => {
  const status = deferred<typeof idle>();
  vi.mocked(command).mockImplementation(async (name) => {
    if (name === 'inspect_host') return host;
    if (name === 'collection_status') return status.promise;
    if (name === 'start_collection') return 'fixture-sensor';
    throw new Error(name);
  });
  await act(async () => {
    render(<HostCollection onLocal={vi.fn()} />);
  });
  fireEvent.change(screen.getByLabelText('Capture interface'), {
    target: { value: 'fixture0' },
  });
  await act(async () => {
    fireEvent.click(screen.getByRole('button', { name: 'Start capture' }));
  });
  expect(screen.queryByText('Capture running')).not.toBeNull();
  await act(async () => status.resolve(idle));
  expect(screen.queryByText('Capture running')).not.toBeNull();
  expect(
    screen.queryByRole('button', { name: 'Stop collection' }),
  ).not.toBeNull();
});

it('does not overlap slow collection status requests', async () => {
  vi.useFakeTimers();
  const status = deferred<typeof idle>();
  vi.mocked(command).mockImplementation(async (name) => {
    if (name === 'inspect_host') return host;
    if (name === 'collection_status') return status.promise;
    throw new Error(name);
  });
  await act(async () => {
    render(<HostCollection onLocal={vi.fn()} />);
  });
  await act(async () => vi.advanceTimersByTimeAsync(5000));
  expect(
    vi
      .mocked(command)
      .mock.calls.filter(([name]) => name === 'collection_status'),
  ).toHaveLength(1);
  await act(async () => status.resolve(idle));
  await act(async () => vi.advanceTimersByTimeAsync(1000));
  expect(
    vi
      .mocked(command)
      .mock.calls.filter(([name]) => name === 'collection_status'),
  ).toHaveLength(2);
});

it('opens the completed collection through the requested view callback', async () => {
  vi.mocked(command).mockImplementation(async (name) => {
    if (name === 'inspect_host') return host;
    if (name === 'collection_status')
      return { ...idle, kind: 'capture', count: 3, sensorId: 'fixture-sensor' };
    throw new Error(name);
  });
  const onLocal = vi.fn();
  const onView = vi.fn();
  await act(async () => {
    render(<HostCollection onLocal={onLocal} onView={onView} />);
  });
  fireEvent.click(
    screen.getByRole('button', { name: 'View these observations' }),
  );
  expect(onView).toHaveBeenCalledWith('fixture-sensor');
  expect(onLocal).not.toHaveBeenCalled();
});

it('blocks source changes during a parent operation while keeping Stop available', async () => {
  vi.mocked(command).mockImplementation(async (name) => {
    if (name === 'inspect_host') return host;
    if (name === 'collection_status')
      return {
        ...idle,
        running: true,
        kind: 'capture',
        sensorId: 'fixture-sensor',
      };
    if (name === 'stop_capture') return;
    throw new Error(name);
  });
  const onView = vi.fn();
  await act(async () => {
    render(<HostCollection parentBusy onLocal={vi.fn()} onView={onView} />);
  });
  const view = screen.getByRole('button', { name: 'View these observations' });
  expect((view as HTMLButtonElement).disabled).toBe(true);
  fireEvent.click(view);
  expect(onView).not.toHaveBeenCalled();
  const stop = screen.getByRole('button', { name: 'Stop collection' });
  expect((stop as HTMLButtonElement).disabled).toBe(false);
  await act(async () => fireEvent.click(stop));
  expect(
    vi.mocked(command).mock.calls.some(([name]) => name === 'stop_capture'),
  ).toBe(true);
});
