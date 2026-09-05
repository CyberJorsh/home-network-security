import { invoke, isTauri } from '@tauri-apps/api/core';
import type { Snapshot } from './types';

export const native = isTauri();
let browserSample: Snapshot | undefined;
export async function readSnapshot(
  mode: string,
  sensor: string | null,
  since: number | null = null,
): Promise<Snapshot> {
  if (native) return invoke<Snapshot>('snapshot', { mode, sensor, since });
  if (!browserSample) {
    const response = await fetch('/sample.json');
    if (!response.ok)
      throw new Error(
        'Sample data could not be loaded. Run the fixture export documented in README.',
      );
    browserSample = (await response.json()) as Snapshot;
  }
  return structuredClone(browserSample!);
}
export async function rename(
  mode: string,
  id: string,
  name: string,
): Promise<void> {
  if (native) return invoke('rename_device', { mode, id, name });
  const d = browserSample?.devices.find((d) => d.id === id);
  if (d) {
    d.name = name;
    d.identification = 'Named by you (sample only)';
  }
}
export async function acknowledge(mode: string, id: string): Promise<void> {
  if (native) return invoke('acknowledge_alert', { mode, id });
  const alert = browserSample?.alerts.find((a) => a.id === id);
  if (alert) alert.acknowledged = true;
}
export async function command<T>(
  name: string,
  args?: Record<string, unknown>,
): Promise<T> {
  if (!native)
    throw new Error(
      'This action requires the desktop app. The browser shows sample data only.',
    );
  return invoke<T>(name, args);
}
