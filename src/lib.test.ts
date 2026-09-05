import { describe, expect, it } from 'vitest';
import { buildSummary, bytes, filterConversations } from './lib';
import type { Snapshot } from './types';

const sample: Snapshot = {
  mode: 'local',
  sensors: [],
  selectedSensor: 'one',
  totals: { upload: 12, download: 20, localBytes: 30, packets: 3 },
  generatedAt: 1,
  networks: ['10.0.0.0/8'],
  observationCount: 3,
  retainedCount: 3,
  limited: false,
  alerts: [],
  timeline: [],
  devices: [
    {
      id: 'secret-id',
      name: 'Private laptop',
      addresses: ['10.0.0.2'],
      mac: '02:11:22:33:44:55',
      category: 'Unknown',
      identification: 'Observed',
      firstSeen: 1,
      lastSeen: 2,
      upload: 12,
      download: 20,
      localBytes: 30,
      connections: 3,
    },
  ],
  conversations: [
    {
      id: 'wan',
      src: '10.0.0.2',
      dst: '203.0.113.2',
      srcDevice: 'secret-id',
      dstDevice: null,
      port: 443,
      protocol: 'TLS',
      direction: 'upload',
      bytes: 12,
      packets: 1,
      firstSeen: 1,
      lastSeen: 2,
      sensorId: 'one',
    },
    {
      id: 'lan',
      src: '10.0.0.2',
      dst: '10.0.0.3',
      srcDevice: 'secret-id',
      dstDevice: null,
      port: 554,
      protocol: 'TCP',
      direction: 'local',
      bytes: 30,
      packets: 1,
      firstSeen: 1,
      lastSeen: 2,
      sensorId: 'one',
    },
    {
      id: 'unrelated',
      src: '10.0.0.99',
      dst: '203.0.113.99',
      srcDevice: 'other',
      dstDevice: null,
      port: 443,
      protocol: 'TLS',
      direction: 'upload',
      bytes: 33,
      packets: 1,
      firstSeen: 1,
      lastSeen: 2,
      sensorId: 'one',
    },
  ],
};
describe('privacy boundary', () => {
  it('redacts names, all endpoint addresses, internal IDs, and MACs', () => {
    const text = buildSummary(sample, 'secret-id');
    for (const secret of [
      'Private laptop',
      'secret-id',
      '02:11:22:33:44:55',
      '10.0.0.2',
      '10.0.0.3',
      '203.0.113.2',
      '203.0.113.99',
    ])
      expect(text).not.toContain(secret);
    expect(text).toContain('endpoint-1');
    expect(text).toContain('E1');
  });
  it('includes only selected alert evidence', () => {
    const text = buildSummary(sample, 'secret-id', false, {
      id: 'a',
      deviceId: 'secret-id',
      title: 'Test',
      severity: 'info',
      detail: 'Observed',
      evidence: ['lan'],
      timestamp: 1,
      acknowledged: false,
    });
    expect(text).toContain('10.0.0.3');
    expect(text).not.toContain('203.0.113.2');
  });
  it('does not leak identifiers hidden inside imported free-form metadata', () => {
    const unusual: Snapshot = {
      ...sample,
      conversations: [
        {
          ...sample.conversations[0],
          protocol: 'private-10.0.0.2',
        },
      ],
    };
    const text = buildSummary(unusual, 'secret-id', true, {
      id: 'a',
      deviceId: 'secret-id',
      title: 'Private laptop 10.0.0.2',
      detail: '02:11:22:33:44:55',
      severity: 'info',
      evidence: ['wan'],
      timestamp: 1,
      acknowledged: false,
    });
    expect(text).not.toContain('10.0.0.2');
    expect(text).not.toContain('Private laptop');
    expect(text).not.toContain('02:11:22:33:44:55');
    expect(text).toContain('Other');
    expect(text).toContain('Selected observation');
  });
  it('requires a selected existing device', () =>
    expect(() => buildSummary(sample, 'missing')).toThrow());
  it('labels sample evidence clearly', () =>
    expect(buildSummary({ ...sample, mode: 'demo' }, 'secret-id')).toContain(
      'SYNTHETIC SAMPLE',
    ));
});
describe('traffic views', () => {
  it('separates internet and local conversations', () => {
    expect(filterConversations(sample, 'local', '')).toHaveLength(1);
    expect(filterConversations(sample, 'internet', '')).toHaveLength(2);
  });
  it('searches names and filters a device together', () =>
    expect(
      filterConversations(sample, 'all', 'Private', 'secret-id'),
    ).toHaveLength(2));
  it('formats units accurately', () => {
    expect(bytes(1024)).toBe('1 KiB');
    expect(bytes(0)).toBe('0 B');
    expect(bytes(-2)).toBe('0 B');
  });
});
