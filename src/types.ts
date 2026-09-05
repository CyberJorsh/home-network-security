export interface Sensor {
  id: string;
  name: string;
  kind: string;
  interface: string;
  internetCoverage: string;
  lanCoverage: string;
  notes: string;
  lastSeen: number | null;
  status: string;
  droppedPackets: number | null;
}
export interface DeviceDetails {
  fieldObservedAt?: Record<string, number>;
  observedAt?: number | null;
  source?: string | null;
  hostname: string | null;
  vendor: string | null;
  model: string | null;
  operatingSystem: string | null;
  services: {
    observedAt?: number | null;
    port: number;
    transport: string;
    name: string | null;
    product: string | null;
    version: string | null;
  }[];
}
export interface Device {
  details?: DeviceDetails;
  id: string;
  name: string;
  addresses: string[];
  mac: string | null;
  category: string;
  identification: string;
  firstSeen: number;
  lastSeen: number;
  upload: number;
  download: number;
  localBytes: number;
  connections: number;
}
export interface Conversation {
  id: string;
  src: string;
  dst: string;
  srcDevice: string | null;
  dstDevice: string | null;
  port: number | null;
  protocol: string;
  direction: 'upload' | 'download' | 'local' | 'multicast' | 'transit';
  bytes: number;
  packets: number;
  firstSeen: number;
  lastSeen: number;
  sensorId: string;
}
export interface Alert {
  id: string;
  deviceId: string;
  severity: 'info' | 'notice';
  title: string;
  detail: string;
  evidence: string[];
  timestamp: number;
  acknowledged: boolean;
}
export interface Bucket {
  timestamp: number;
  upload: number;
  download: number;
  localBytes: number;
}
export interface Snapshot {
  bucketSeconds?: number;
  mode: 'demo' | 'local' | 'collector';
  sensors: Sensor[];
  selectedSensor: string | null;
  devices: Device[];
  conversations: Conversation[];
  alerts: Alert[];
  timeline: Bucket[];
  totals: {
    upload: number;
    download: number;
    localBytes: number;
    packets: number;
  };
  networks: string[];
  observationCount: number;
  retainedCount: number;
  limited: boolean;
  generatedAt: number;
}
export type Provider = 'chatgpt' | 'grok';
export type Page =
  'overview' | 'devices' | 'traffic' | 'alerts' | 'collection' | 'assistant';
