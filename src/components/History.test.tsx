// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest';
import { cleanup, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import ExplanationHistory from './ExplanationHistory';
import StorageControls from './StorageControls';
import { command } from '../api';
vi.mock('../api', () => ({ native: true, command: vi.fn() }));
afterEach(() => {
  cleanup();
  vi.mocked(command).mockReset();
});
it('saves only on an explicit click and reloads the exact saved summary', async () => {
  const saved = {
    id: 'fixture',
    savedAt: 1,
    body: {
      provider: 'chatgpt',
      model: 'fixture-model',
      summary: 'Exact reviewed fixture',
      text: 'Synthetic response',
    },
  };
  let items: (typeof saved)[] = [];
  vi.mocked(command).mockImplementation(async (name) => {
    if (name === 'explanation_history') return items;
    if (name === 'save_explanation') {
      items = [saved];
      return;
    }
    if (name === 'delete_explanation') {
      items = [];
      return;
    }
    throw new Error(name);
  });
  const user = userEvent.setup();
  render(
    <ExplanationHistory
      canSave
      onError={(e) => {
        throw new Error(e);
      }}
    />,
  );
  expect(vi.mocked(command).mock.calls).toHaveLength(0);
  await user.click(screen.getByText('Saved explanations · on this computer'));
  await user.click(
    screen.getByRole('button', { name: 'Save completed explanation' }),
  );
  await user.click(await screen.findByText(/fixture-model/));
  await user.click(screen.getByText('Submitted summary'));
  expect(screen.queryByText('Exact reviewed fixture')).not.toBeNull();
  await user.click(
    screen.getByRole('button', { name: 'Delete saved explanation' }),
  );
  await screen.findByText('No saved explanations.');
});
it('does not invoke deletion when opening or cancelling the confirmation', async () => {
  vi.mocked(command).mockImplementation(async (name) =>
    name === 'storage_limit' ? 100000 : true,
  );
  const changed = vi.fn();
  const user = userEvent.setup();
  render(
    <StorageControls
      onChanged={changed}
      onError={(e) => {
        throw new Error(e);
      }}
    />,
  );
  await user.click(screen.getByText('Local history and storage'));
  await user.click(screen.getByRole('button', { name: 'Delete local data…' }));
  const button = screen.getByRole('button', {
    name: 'Permanently delete local data',
  }) as HTMLButtonElement;
  expect(button.disabled).toBe(true);
  await user.type(
    screen.getByLabelText('Type DELETE LOCAL DATA to confirm'),
    'DELETE',
  );
  expect(button.disabled).toBe(true);
  expect(
    vi.mocked(command).mock.calls.some(([n]) => n === 'clear_local_data'),
  ).toBe(false);
  await user.click(screen.getByRole('button', { name: 'Cancel deletion' }));
  expect(
    screen.queryByRole('region', { name: 'Confirm local data deletion' }),
  ).toBeNull();
  expect(changed).not.toHaveBeenCalled();
});
it('passes exact typed confirmation only on the final delete click', async () => {
  vi.mocked(command).mockImplementation(async (name) =>
    name === 'storage_limit' ? 100000 : true,
  );
  const changed = vi.fn();
  const user = userEvent.setup();
  render(
    <StorageControls
      onChanged={changed}
      onError={(e) => {
        throw new Error(e);
      }}
    />,
  );
  await user.click(screen.getByText('Local history and storage'));
  await user.click(screen.getByRole('button', { name: 'Delete local data…' }));
  await user.type(
    screen.getByLabelText('Type DELETE LOCAL DATA to confirm'),
    'DELETE LOCAL DATA',
  );
  expect(
    vi.mocked(command).mock.calls.some(([n]) => n === 'clear_local_data'),
  ).toBe(false);
  await user.click(
    screen.getByRole('button', { name: 'Permanently delete local data' }),
  );
  await waitFor(() => expect(changed).toHaveBeenCalledTimes(1));
  expect(
    vi.mocked(command).mock.calls.find(([n]) => n === 'clear_local_data')?.[1],
  ).toEqual({ confirmation: 'DELETE LOCAL DATA' });
});
