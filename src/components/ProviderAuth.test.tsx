// @vitest-environment jsdom
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
} from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { command } from '../api';
import ProviderAuth, { type Auth } from './ProviderAuth';

vi.mock('../api', () => ({ native: true, command: vi.fn() }));
const connected: Auth = {
  busy: false,
  signedIn: true,
  message: 'Available',
  loginUrl: null,
  account: 'fixture account',
  plan: null,
  clientVersion: null,
};
beforeEach(() => {
  vi.useFakeTimers();
  vi.mocked(command).mockReset();
});
afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

it('does not overlap slow session status polls', async () => {
  vi.mocked(command).mockImplementation(() => new Promise(() => {}));
  render(<ProviderAuth provider="chatgpt" onStatus={() => {}} />);
  await act(() => vi.advanceTimersByTimeAsync(3500));
  expect(vi.mocked(command).mock.calls).toHaveLength(1);
});

it('clears a transient polling error when the session recovers', async () => {
  vi.mocked(command)
    .mockRejectedValueOnce(new Error('Temporary session read failure'))
    .mockResolvedValue(connected);
  render(<ProviderAuth provider="chatgpt" onStatus={() => {}} />);
  await act(async () => {});
  expect(screen.queryByRole('alert')?.textContent).toContain('Temporary');
  await act(() => vi.advanceTimersByTimeAsync(700));
  expect(screen.queryByText('ChatGPT connected')).not.toBeNull();
  expect(screen.queryByRole('alert')).toBeNull();
});

it('keeps an account action error visible during successful background polls', async () => {
  vi.mocked(command).mockImplementation(async (name) => {
    if (name === 'auth_action') throw new Error('Session check failed');
    return connected;
  });
  render(<ProviderAuth provider="chatgpt" onStatus={() => {}} />);
  await act(async () => {});
  await act(async () =>
    fireEvent.click(screen.getByRole('button', { name: 'Check session' })),
  );
  await act(() => vi.advanceTimersByTimeAsync(1500));
  expect(screen.queryByRole('alert')?.textContent).toContain(
    'Session check failed',
  );
});

it('ignores an account action that finishes after leaving its provider', async () => {
  let finish!: () => void;
  vi.mocked(command).mockImplementation(async (name) => {
    if (name === 'auth_status') return connected;
    return new Promise<void>((resolve) => {
      finish = resolve;
    });
  });
  const onStatus = vi.fn();
  const view = render(<ProviderAuth provider="chatgpt" onStatus={onStatus} />);
  await act(async () => {});
  fireEvent.click(screen.getByRole('button', { name: 'Check session' }));
  view.unmount();
  onStatus.mockClear();
  await act(async () => finish());
  expect(onStatus).not.toHaveBeenCalled();
  expect(
    vi.mocked(command).mock.calls.filter(([name]) => name === 'auth_status'),
  ).toHaveLength(1);
});

it('does not let a status read from before an account action restore the old session', async () => {
  const stale: ((auth: Auth) => void)[] = [];
  let reads = 0;
  let acted = false;
  vi.mocked(command).mockImplementation(async (name) => {
    if (name === 'auth_action') {
      acted = true;
      return;
    }
    if (acted) return { ...connected, signedIn: false, message: 'Signed out' };
    if (reads++ === 0) return connected;
    return new Promise<Auth>((resolve) => stale.push(resolve));
  });
  render(<ProviderAuth provider="chatgpt" onStatus={() => {}} />);
  await act(async () => {});
  await act(() => vi.advanceTimersByTimeAsync(1500));
  expect(stale.length).toBeGreaterThan(0);
  await act(async () =>
    fireEvent.click(screen.getByRole('button', { name: 'Check session' })),
  );
  expect(screen.queryByText('ChatGPT connected')).toBeNull();
  await act(async () => stale.forEach((resolve) => resolve(connected)));
  expect(screen.queryByText('ChatGPT connected')).toBeNull();
  expect(screen.queryByText('Signed out')).not.toBeNull();
});
