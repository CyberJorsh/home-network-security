// @vitest-environment jsdom
import { act, cleanup, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { command } from '../api';
import ExplanationHistory from './ExplanationHistory';

vi.mock('../api', () => ({ native: true, command: vi.fn() }));
const saved = [
  {
    id: 'fixture',
    savedAt: 1,
    body: {
      provider: 'chatgpt',
      model: 'fixture model',
      summary: 'Synthetic summary',
      text: 'Synthetic saved response',
    },
  },
];
beforeEach(() => vi.mocked(command).mockReset());
afterEach(cleanup);

it('does not restore a deleted explanation from an older history refresh', async () => {
  let deleted = false;
  let reads = 0;
  const stale: ((items: typeof saved) => void)[] = [];
  vi.mocked(command).mockImplementation(async (name) => {
    if (name === 'delete_explanation') {
      deleted = true;
      return;
    }
    if (deleted) return [];
    if (reads++ === 0) return saved;
    return new Promise((resolve) => stale.push(resolve));
  });
  const user = userEvent.setup();
  render(<ExplanationHistory canSave={false} onError={() => {}} />);
  const heading = screen.getByText('Saved explanations · on this computer');
  await user.click(heading);
  await screen.findByText('Synthetic saved response');
  await user.click(heading);
  await user.click(heading);
  await waitFor(() => expect(stale.length).toBeGreaterThan(0));
  await user.click(screen.getByText(/chatgpt · fixture model/));
  await user.click(
    screen.getByRole('button', { name: 'Delete saved explanation' }),
  );
  await screen.findByText('No saved explanations.');
  await act(async () => stale.forEach((resolve) => resolve(saved)));
  expect(screen.queryByText('Synthetic saved response')).toBeNull();
});

it('does not refresh history when expanding an individual saved response', async () => {
  vi.mocked(command).mockResolvedValue(saved);
  const user = userEvent.setup();
  render(<ExplanationHistory canSave={false} onError={() => {}} />);
  await user.click(screen.getByText('Saved explanations · on this computer'));
  await screen.findByText('Synthetic saved response');
  vi.mocked(command).mockClear();
  await user.click(screen.getByText(/chatgpt · fixture model/));
  await user.click(screen.getByText('Submitted summary'));
  await act(async () => {});
  expect(command).not.toHaveBeenCalled();
});
