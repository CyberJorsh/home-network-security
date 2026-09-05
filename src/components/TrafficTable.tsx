import {
  ArrowDownLeft,
  ArrowRight,
  ArrowUpRight,
  ArrowLeftRight,
} from 'lucide-react';
import type { Conversation, Device } from '../types';
import { bytes, deviceLabel, time } from '../lib';

export default function TrafficTable({
  flows,
  devices,
  onDevice,
}: {
  flows: Conversation[];
  devices: Device[];
  onDevice: (id: string) => void;
}) {
  return (
    <div className="table-scroll">
      <table className="traffic-table">
        <thead>
          <tr>
            <th>Connection</th>
            <th>Protocol</th>
            <th>Direction</th>
            <th>Volume</th>
            <th>Last seen</th>
          </tr>
        </thead>
        <tbody>
          {flows.slice(0, 200).map((c) => (
            <tr key={c.id} id={c.id}>
              <td>
                <div className="connection-pair">
                  {c.srcDevice ? (
                    <button
                      className="text-button"
                      onClick={() => onDevice(c.srcDevice!)}
                    >
                      {deviceLabel(c.srcDevice, c.src, devices)}
                    </button>
                  ) : (
                    <span>{c.src}</span>
                  )}
                  <ArrowRight size={13} />
                  {c.dstDevice ? (
                    <button
                      className="text-button"
                      onClick={() => onDevice(c.dstDevice!)}
                    >
                      {deviceLabel(c.dstDevice, c.dst, devices)}
                    </button>
                  ) : (
                    <span>{c.dst}</span>
                  )}
                </div>
                <small>
                  {c.src} → {c.dst}
                  {c.port !== null ? `:${c.port}` : ''}
                </small>
              </td>
              <td>
                <span className="protocol">{c.protocol}</span>
              </td>
              <td>
                <span className={`direction ${c.direction}`}>
                  {c.direction === 'upload' ? (
                    <ArrowUpRight size={14} />
                  ) : c.direction === 'download' ? (
                    <ArrowDownLeft size={14} />
                  ) : (
                    <ArrowLeftRight size={14} />
                  )}
                  {c.direction === 'local'
                    ? 'Local'
                    : c.direction === 'upload'
                      ? 'Upload'
                      : c.direction === 'download'
                        ? 'Download'
                        : c.direction}
                </span>
              </td>
              <td className="numeric">{bytes(c.bytes)}</td>
              <td className="muted">{time(c.lastSeen)}</td>
            </tr>
          ))}
          {!flows.length && (
            <tr>
              <td colSpan={5} className="empty-row">
                No connections match this view.
              </td>
            </tr>
          )}
        </tbody>
      </table>
      {flows.length > 200 && (
        <p className="table-note">
          Showing the 200 largest of {flows.length} connections. Refine your
          search to see others.
        </p>
      )}
    </div>
  );
}
