// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest';
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
} from '@testing-library/react';
import App from './App';
import { command, readSnapshot } from './api';
import sample from '../public/sample.json';
import type { Snapshot } from './types';

vi.mock('./api', () => ({
  native: true,
  readSnapshot: vi.fn(),
  command: vi.fn(),
  rename: vi.fn(),
  acknowledge: vi.fn(),
}));
vi.mock('./components/HostCollection', () => ({
  default: ({ onView }: { onView: (id: string) => void }) => (
    <button onClick={() => onView('local-fixture')}>
      View these observations
    </button>
  ),
}));
const fixture: Snapshot = {
  ...(sample as Snapshot),
  sensors: [
    sample.sensors[0],
    { ...sample.sensors[0], id: 'other', name: 'Other source' },
  ],
  conversations: [],
};
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
  vi.useRealTimers();
});

it('hides the old source while a different source is loading', async () => {
  vi.mocked(readSnapshot).mockResolvedValue(fixture);
  render(<App />);
  await screen.findByText(fixture.devices[0].name);
  vi.mocked(readSnapshot).mockImplementation(() => new Promise(() => {}));
  fireEvent.change(screen.getByLabelText('Observation source'), {
    target: { value: fixture.sensors[1].id },
  });
  expect(screen.queryByText(fixture.devices[0].name)).toBeNull();
  expect(screen.getByText('Loading observations…')).toBeTruthy();
});

it('waits for a slow read to finish before scheduling another poll', async () => {
  vi.useFakeTimers();
  let finish!: (value: Snapshot) => void;
  vi.mocked(readSnapshot).mockImplementation(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  );
  render(<App />);
  await act(async () => {
    await vi.advanceTimersByTimeAsync(30000);
  });
  expect(readSnapshot).toHaveBeenCalledTimes(1);
  await act(async () => {
    finish(fixture);
  });
  expect(screen.getByText(fixture.devices[0].name)).toBeTruthy();
});

it('opens the device results when a completed collection is selected', async () => {
  vi.mocked(readSnapshot).mockResolvedValue(fixture);
  vi.mocked(command).mockResolvedValue(undefined);
  window.scrollTo = vi.fn();
  render(<App />);
  await screen.findByText(fixture.devices[0].name);
  fireEvent.click(screen.getByRole('button', { name: 'Collection' }));
  fireEvent.click(
    screen.getByRole('button', { name: 'View these observations' }),
  );
  await screen.findByRole('heading', { name: 'Make yourself familiar.' });
  expect(readSnapshot).toHaveBeenLastCalledWith('local', 'local-fixture', null);
});

it('can recover to local data when the first snapshot from a source fails', async () => {
  vi.mocked(readSnapshot).mockRejectedValueOnce(
    new Error('Collector unavailable'),
  );
  vi.mocked(command).mockResolvedValue(undefined);
  render(<App />);
  await screen.findByText('Couldn’t load observations.');
  vi.mocked(readSnapshot).mockResolvedValue(fixture);
  fireEvent.click(
    screen.getByRole('button', { name: 'Use local observations' }),
  );
  await screen.findByText(fixture.devices[0].name);
  expect(command).toHaveBeenCalledWith('disconnect_collector');
  expect(readSnapshot).toHaveBeenLastCalledWith('local', null, null);
});
