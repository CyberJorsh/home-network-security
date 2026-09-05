// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import Assistant from './Assistant';
import HostCollection from './HostCollection';
import { command } from '../api';
import type { Snapshot } from '../types';
vi.mock('../api', () => ({ native: true, command: vi.fn() }));
const fixture: Snapshot = {
  mode: 'demo',
  sensors: [],
  selectedSensor: null,
  devices: [
    {
      id: 'device',
      name: 'Fixture laptop',
      addresses: ['192.0.2.1'],
      mac: '02:00:00:00:00:01',
      category: 'Unknown',
      identification: 'Synthetic',
      firstSeen: 1,
      lastSeen: 2,
      upload: 0,
      download: 0,
      localBytes: 0,
      connections: 0,
    },
  ],
  conversations: [],
  alerts: [],
  timeline: [],
  totals: { upload: 0, download: 0, localBytes: 0, packets: 0 },
  networks: [],
  observationCount: 0,
  retainedCount: 0,
  limited: false,
  generatedAt: 2,
};
const catalog = {
  models: [
    {
      id: 'fixture-model',
      name: 'Fixture model',
      description: '',
      efforts: ['low', 'high'],
      defaultEffort: 'low',
      isDefault: true,
    },
  ],
  allowance: 'Available',
};
let signedIn = true;
let missing = false;
let response = {
  running: false,
  provider: 'chatgpt',
  model: 'fixture-model',
  text: '',
  error: null,
  completed: false,
};
beforeEach(() => {
  localStorage.clear();
  signedIn = true;
  missing = false;
  response = { ...response, running: false, text: '', completed: false };
  vi.mocked(command)
    .mockReset()
    .mockImplementation(async (name, args) => {
      if (name === 'auth_status')
        return {
          busy: false,
          signedIn,
          message: signedIn ? 'Available' : '',
          account: null,
          plan: null,
          loginUrl: null,
          clientVersion: null,
        };
      if (name === 'auth_action') {
        signedIn = true;
        return;
      }
      if (name === 'provider_models') return catalog;
      if (name === 'explanation_status') return response;
      if (name === 'send_explanation') {
        response = { ...response, running: true, text: 'First streamed words' };
        return;
      }
      if (name === 'inspect_host')
        return {
          interfaces: missing
            ? []
            : [{ id: 'fixture0', label: 'Fixture interface' }],
          addresses: [],
          suggestedCidrs: ['10.0.0.0/24'],
          captureError: missing ? 'Not installed' : null,
          captureRemedy: missing ? 'install' : null,
          discoveryAvailable: !missing,
          platform: 'macos',
        };
      if (name === 'collection_status')
        return {
          running: false,
          kind: '',
          count: 0,
          sensorId: null,
          error: null,
        };
      if (name === 'install_collection_tool') return true;
      throw new Error(`Unexpected command ${name}: ${JSON.stringify(args)}`);
    });
});
afterEach(() => cleanup());
const sent = () =>
  vi.mocked(command).mock.calls.filter(([name]) => name === 'send_explanation');
function mount() {
  render(
    <Assistant
      snapshot={fixture}
      initialDevice="device"
      onNotice={() => {}}
      onError={() => {}}
    />,
  );
}
describe('reviewed subscription explanations', () => {
  it('restores a session and hides redundant sign-in controls', async () => {
    signedIn = false;
    mount();
    await waitFor(() =>
      expect(screen.queryByText('ChatGPT connected')).not.toBeNull(),
    );
    expect(
      vi
        .mocked(command)
        .mock.calls.some(
          ([name, args]) => name === 'auth_action' && args?.action === 'check',
        ),
    ).toBe(true);
    expect(
      screen.queryByRole('button', { name: 'Sign in to ChatGPT' }),
    ).toBeNull();
    expect(sent()).toHaveLength(0);
  });
  it('sends exactly edited reviewed text once and displays streamed output', async () => {
    const user = userEvent.setup();
    mount();
    await screen.findByRole('option', { name: 'Fixture model' });
    await user.click(screen.getByRole('button', { name: 'Prepare summary' }));
    const button = screen.getByRole('button', {
      name: 'Send to ChatGPT',
    }) as HTMLButtonElement;
    expect(button.disabled).toBe(true);
    const editor = screen.getByRole('textbox', { name: 'Editable summary' });
    await user.clear(editor);
    await user.type(editor, 'Reviewed synthetic fixture only.');
    await user.click(
      screen.getByRole('checkbox', { name: 'I reviewed this exact summary.' }),
    );
    await user.selectOptions(screen.getByLabelText('Reasoning effort'), 'high');
    expect(button.disabled).toBe(true);
    await user.click(
      screen.getByRole('checkbox', { name: 'I reviewed this exact summary.' }),
    );
    await user.click(button);
    expect(sent()).toHaveLength(1);
    expect(sent()[0][1]).toEqual({
      request: {
        provider: 'chatgpt',
        model: 'fixture-model',
        effort: 'high',
        text: 'Reviewed synthetic fixture only.',
        reviewed: true,
      },
    });
    await screen.findByText('First streamed words');
    expect(
      screen.queryByRole('checkbox', {
        name: 'I reviewed this exact summary.',
      }),
    ).toBeNull();
    expect(
      screen.queryByRole('button', { name: 'Copy reviewed summary' }),
    ).toBeNull();
    expect(screen.queryByRole('button', { name: 'Open ChatGPT' })).toBeNull();
  });
  it('resets review when the provider changes', async () => {
    const user = userEvent.setup();
    mount();
    await screen.findByRole('option', { name: 'Fixture model' });
    await user.click(screen.getByRole('button', { name: 'Prepare summary' }));
    await user.click(
      screen.getByRole('checkbox', { name: 'I reviewed this exact summary.' }),
    );
    await user.click(screen.getByRole('button', { name: 'Grok' }));
    expect(
      (
        screen.getByRole('checkbox', {
          name: 'I reviewed this exact summary.',
        }) as HTMLInputElement
      ).checked,
    ).toBe(false);
    expect(sent()).toHaveLength(0);
  });
});
describe('collection dependency setup', () => {
  it.each(['Discover devices', 'Start capture'])(
    'offers installation from %s when tools are absent',
    async (label) => {
      missing = true;
      const user = userEvent.setup();
      render(<HostCollection onLocal={() => {}} />);
      await screen.findByText('Not installed');
      await user.click(screen.getByRole('button', { name: label }));
      await screen.findByText('Finish installation in the terminal');
      expect(
        vi
          .mocked(command)
          .mock.calls.some(
            ([name, args]) =>
              name === 'install_collection_tool' &&
              args?.tool === (label === 'Discover devices' ? 'nmap' : 'tshark'),
          ),
      ).toBe(true);
      expect(
        vi
          .mocked(command)
          .mock.calls.some(([name]) => name === 'start_collection'),
      ).toBe(false);
    },
  );
});

describe('recovery and saved preferences', () => {
  it('waits for a running provider, then loads models without a manual refresh', async () => {
    const original = vi.mocked(command).getMockImplementation()!;
    let providerBusy = true;
    vi.mocked(command).mockImplementation(async (name, args) =>
      name === 'auth_status'
        ? { ...((await original(name, args)) as object), busy: providerBusy }
        : original(name, args),
    );
    mount();
    await screen.findByText('ChatGPT connected');
    expect(
      vi.mocked(command).mock.calls.filter(([n]) => n === 'provider_models'),
    ).toHaveLength(0);
    providerBusy = false;
    await screen.findByRole(
      'option',
      { name: 'Fixture model' },
      { timeout: 2500 },
    );
    expect(
      vi.mocked(command).mock.calls.filter(([n]) => n === 'provider_models'),
    ).toHaveLength(1);
  });
  it('remembers provider and effort after leaving and returning', async () => {
    const user = userEvent.setup();
    mount();
    await screen.findByRole('option', { name: 'Fixture model' });
    await user.click(screen.getByRole('button', { name: 'Grok' }));
    await screen.findByText('Grok connected');
    await screen.findByRole('option', { name: 'Fixture model' });
    await user.selectOptions(screen.getByLabelText('Reasoning effort'), 'high');
    cleanup();
    mount();
    await screen.findByText('Grok connected');
    await screen.findByRole('option', { name: 'Fixture model' });
    expect(
      (screen.getByLabelText('Reasoning effort') as HTMLSelectElement).value,
    ).toBe('high');
  });
  it('does not discard the catalog when its own request makes the provider busy', async () => {
    const original = vi.mocked(command).getMockImplementation()!;
    let providerBusy = false;
    let finish: ((value: typeof catalog) => void) | undefined;
    vi.mocked(command).mockImplementation(async (name, args) => {
      if (name === 'auth_status')
        return {
          ...((await original(name, args)) as object),
          busy: providerBusy,
        };
      if (name === 'provider_models') {
        providerBusy = true;
        return new Promise((resolve) => {
          finish = resolve;
        });
      }
      return original(name, args);
    });
    mount();
    await waitFor(() => expect(finish).toBeDefined());
    await new Promise((resolve) => setTimeout(resolve, 800));
    finish!(catalog);
    await screen.findByRole('option', { name: 'Fixture model' });
    expect(
      vi.mocked(command).mock.calls.filter(([n]) => n === 'provider_models'),
    ).toHaveLength(1);
  });
});
describe('capture failure diagnosis', () => {
  it.each(['permission', 'refresh'] as const)(
    'does not reinstall TShark for a %s failure',
    async (remedy) => {
      const original = vi.mocked(command).getMockImplementation()!;
      vi.mocked(command).mockImplementation(async (name, args) =>
        name === 'inspect_host'
          ? {
              ...((await original(name, args)) as object),
              platform: 'windows',
              captureError: 'Interface check failed',
              captureRemedy: remedy,
            }
          : original(name, args),
      );
      const user = userEvent.setup();
      render(<HostCollection onLocal={() => {}} />);
      await screen.findByText('Interface check failed');
      await user.click(screen.getByRole('button', { name: 'Start capture' }));
      expect(
        vi
          .mocked(command)
          .mock.calls.filter(([n]) => n === 'install_collection_tool'),
      ).toHaveLength(0);
      expect(
        screen.queryByText(/Refresh interfaces after checking/),
      ).not.toBeNull();
    },
  );
  it('resumes discovery after installation is detected', async () => {
    missing = true;
    const original = vi.mocked(command).getMockImplementation()!;
    vi.mocked(command).mockImplementation(async (name, args) =>
      name === 'start_collection' ? 'fixture-sensor' : original(name, args),
    );
    const user = userEvent.setup();
    const onLocal = vi.fn();
    render(<HostCollection onLocal={onLocal} />);
    await screen.findByText('Not installed');
    await user.click(screen.getByRole('button', { name: 'Discover devices' }));
    await screen.findByText('Finish installation in the terminal');
    missing = false;
    await user.click(
      screen.getByRole('button', { name: 'Continue after installation' }),
    );
    await waitFor(() => expect(onLocal).toHaveBeenCalledWith('fixture-sensor'));
    expect(
      vi.mocked(command).mock.calls.filter(([n]) => n === 'start_collection'),
    ).toHaveLength(1);
  });
});
