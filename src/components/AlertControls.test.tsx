// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import AlertControls from './AlertControls';
import { command } from '../api';
vi.mock('../api', () => ({ native: true, command: vi.fn() }));
afterEach(() => {
  cleanup();
  vi.mocked(command).mockReset();
});
it('loads persisted rules, rejects invalid limits and saves only on request', async () => {
  vi.mocked(command).mockImplementation(async (name) =>
    name === 'alert_rules'
      ? { uploadEnabled: true, uploadThresholdMib: 75 }
      : undefined,
  );
  const changed = vi.fn();
  const user = userEvent.setup();
  render(
    <AlertControls
      onChanged={changed}
      onError={(e) => {
        throw new Error(e);
      }}
    />,
  );
  await user.click(screen.getByText('Local alert rules'));
  const input = await screen.findByDisplayValue('75');
  expect(vi.mocked(command).mock.calls).toHaveLength(1);
  await user.clear(input);
  await user.type(input, '0');
  expect(
    (
      screen.getByRole('button', {
        name: 'Save alert rules',
      }) as HTMLButtonElement
    ).disabled,
  ).toBe(true);
  await user.clear(input);
  await user.type(input, '1');
  await user.click(screen.getByLabelText('Enable large upload notices'));
  await user.click(screen.getByRole('button', { name: 'Save alert rules' }));
  expect(command).toHaveBeenCalledWith('set_alert_rules', {
    rules: { uploadEnabled: false, uploadThresholdMib: 1 },
  });
  expect(changed).toHaveBeenCalledOnce();
  expect(
    await screen.findByText('Alert rules saved for this computer.'),
  ).not.toBeNull();
});
