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
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
  vi.useRealTimers();
});

it('keeps capture status and Stop available before the sensor has been registered', async () => {
  vi.useFakeTimers();
  window.scrollTo = vi.fn();
  vi.mocked(readSnapshot).mockImplementation(async (_mode, sensor) => {
    if (sensor === 'new-sensor') throw new Error('Unknown sensor');
    return sample as Snapshot;
  });
  let running = false;
  vi.mocked(command).mockImplementation(async (name) => {
    if (name === 'inspect_host')
      return {
        interfaces: [{ id: 'fixture0', label: 'Fixture interface' }],
        addresses: [],
        suggestedCidrs: [],
        captureError: null,
        discoveryAvailable: true,
        platform: 'macos',
      };
    if (name === 'collection_status')
      return {
        running,
        kind: running ? 'capture' : '',
        count: 0,
        sensorId: running ? 'new-sensor' : null,
        error: null,
      };
    if (name === 'start_collection') {
      running = true;
      return 'new-sensor';
    }
    if (name === 'stop_capture') running = false;
    return undefined;
  });
  await act(async () => {
    render(<App />);
  });
  await act(async () => {
    fireEvent.click(screen.getByRole('button', { name: 'Collection' }));
  });
  fireEvent.change(screen.getByLabelText('Capture interface'), {
    target: { value: 'fixture0' },
  });
  await act(async () => {
    fireEvent.click(screen.getByRole('button', { name: 'Start capture' }));
  });
  expect(screen.getByText('Couldn’t load observations.')).toBeTruthy();
  expect(screen.getByText('Capture running')).toBeTruthy();
  fireEvent.click(screen.getByRole('button', { name: 'Stop collection' }));
  expect(command).toHaveBeenCalledWith('stop_capture');
  expect(
    vi.mocked(command).mock.calls.filter(([name]) => name === 'inspect_host'),
  ).toHaveLength(1);
});
