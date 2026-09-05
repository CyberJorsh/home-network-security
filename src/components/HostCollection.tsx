import { useEffect, useState } from 'react';
import { command, native } from '../api';

type Host = {
  interfaces: { id: string; label: string }[];
  addresses: string[];
  suggestedCidrs: string[];
  captureError: string | null;
  discoveryAvailable: boolean;
  platform: string;
};
type Job = {
  running: boolean;
  kind: string;
  count: number;
  sensorId: string | null;
  error: string | null;
};
export default function HostCollection({
  onLocal,
}: {
  onLocal: (sensor: string) => void;
}) {
  const [host, setHost] = useState<Host>();
  const [job, setJob] = useState<Job>();
  const [cidr, setCidr] = useState('');
  const [device, setDevice] = useState('');
  const [seconds, setSeconds] = useState('60');
  const [error, setError] = useState('');
  const [starting, setStarting] = useState(false);
  const [checking, setChecking] = useState(false);
  const inspect = async () => {
    setChecking(true);
    setError('');
    try {
      const result = await command<Host>('inspect_host');
      setHost(result);
      setCidr((value) => value || result.suggestedCidrs[0] || '');
      setDevice((value) =>
        result.interfaces.some((i) => i.id === value) ? value : '',
      );
    } catch (e) {
      setError(String(e));
    } finally {
      setChecking(false);
    }
  };
  useEffect(() => {
    if (!native) return;
    let alive = true;
    void command<Host>('inspect_host')
      .then((result) => {
        if (alive) {
          setHost(result);
          setCidr(result.suggestedCidrs[0] || '');
        }
      })
      .catch((e) => {
        if (alive) setError(String(e));
      });
    const poll = () =>
      void command<Job>('collection_status')
        .then((result) => {
          if (alive) setJob(result);
        })
        .catch((e) => {
          if (alive) setError(String(e));
        });
    poll();
    const interval = setInterval(poll, 1000);
    return () => {
      alive = false;
      clearInterval(interval);
    };
  }, []);
  const start = async (kind: 'discover' | 'capture') => {
    setStarting(true);
    setError('');
    try {
      const sensor = await command<string>('start_collection', {
        kind,
        target: kind === 'discover' ? cidr : device,
        seconds: Number(seconds),
      });
      setJob({ running: true, kind, count: 0, sensorId: sensor, error: null });
      onLocal(sensor);
    } catch (e) {
      setError(String(e));
    } finally {
      setStarting(false);
    }
  };
  const busy = starting || job?.running;
  return (
    <section className="panel setup-panel host-collection">
      <div className="panel-heading compact">
        <h2>Scan from this computer</h2>
        <span className="pill">Local only</span>
      </div>
      <p>
        Discover devices on a selected home network, then capture IP traffic
        visible on one interface. Results stay in this computer’s database. This
        switches the dashboard to local observations.
      </p>
      {!native && (
        <p className="integration-status">
          Open the desktop app to sign in or collect real data. This browser is
          a sample preview.
        </p>
      )}
      <div className="button-row">
        <button
          className="button secondary"
          disabled={!native || checking}
          onClick={() => void inspect()}
        >
          {checking ? 'Checking tools…' : 'Refresh interfaces and tools'}
        </button>
      </div>
      {host && (
        <>
          <p className="hint">
            Nmap: {host.discoveryAvailable ? 'available' : 'not installed'} ·
            TShark:{' '}
            {host.captureError
              ? 'unavailable'
              : 'available (capture permission checked when started)'}
          </p>
          <details className="setup-details">
            <summary>This computer’s addresses</summary>
            {host.addresses.map((address) => (
              <code key={address}>{address}</code>
            ))}
          </details>
          {host.captureError && <p role="alert">{host.captureError}</p>}
        </>
      )}
      <div className="host-controls">
        <form
          onSubmit={(e) => {
            e.preventDefault();
            void start('discover');
          }}
        >
          <h3>Discover nearby devices</h3>
          <p>
            Send discovery probes to a private IPv4 /24 or smaller. A suggested
            range covers up to 256 addresses around a local interface.
          </p>
          <label htmlFor="scan-cidr">Network to scan</label>
          <input
            id="scan-cidr"
            list="local-cidrs"
            value={cidr}
            onChange={(e) => setCidr(e.target.value)}
            placeholder="192.168.1.0/24"
            required
          />
          <datalist id="local-cidrs">
            {host?.suggestedCidrs.map((value) => (
              <option key={value} value={value} />
            ))}
          </datalist>
          <button
            className="button primary"
            disabled={
              !native || busy || !host?.discoveryAvailable || !cidr.trim()
            }
          >
            Discover devices
          </button>
        </form>
        <form
          onSubmit={(e) => {
            e.preventDefault();
            void start('capture');
          }}
        >
          <h3>Capture visible traffic</h3>
          <p>
            Records connection metadata. Raw packet payloads are not saved.
            Other devices’ private conversations may not reach this interface.
          </p>
          <label htmlFor="capture-interface">Capture interface</label>
          <select
            id="capture-interface"
            value={device}
            onChange={(e) => setDevice(e.target.value)}
            required
          >
            <option value="">Choose an interface</option>
            {host?.interfaces.map((i) => (
              <option key={i.id} value={i.id}>
                {i.label}
              </option>
            ))}
          </select>
          <label htmlFor="capture-seconds">Duration in seconds</label>
          <input
            id="capture-seconds"
            type="number"
            min="1"
            max="86400"
            value={seconds}
            onChange={(e) => setSeconds(e.target.value)}
            required
          />
          <button
            className="button primary"
            disabled={!native || busy || !device}
          >
            Start capture
          </button>
        </form>
      </div>
      {job?.kind && (
        <div className="integration-status" role="status">
          <strong>
            {job.kind === 'discover' ? 'Discovery' : 'Capture'}{' '}
            {job.running ? 'running' : job.error ? 'failed' : 'finished'}
          </strong>
          <p>
            {job.count.toLocaleString()}{' '}
            {job.kind === 'discover'
              ? 'devices found'
              : 'IP observations saved'}
            .
            {job.running && job.kind === 'discover'
              ? ' Discovery can take up to five minutes.'
              : ''}
          </p>
          {job.error && (
            <p role="alert" className="tool-output">
              {job.error}
            </p>
          )}
          <div className="button-row">
            {job.running && (
              <button
                className="button secondary"
                onClick={() =>
                  void command('stop_capture').catch((e) => setError(String(e)))
                }
              >
                Stop collection
              </button>
            )}
            {job.sensorId && (
              <button
                className="button secondary"
                onClick={() => onLocal(job.sensorId!)}
              >
                View these observations
              </button>
            )}
          </div>
        </div>
      )}
      {error && <p role="alert">{error}</p>}
      <details className="setup-details">
        <summary>Install tools and enable capture</summary>
        <p>
          macOS: install Nmap and Wireshark CLI tools with Homebrew. If capture
          reports permission denied, install Wireshark’s official ChmodBPF
          helper and complete the macOS administrator prompt.
        </p>
        <code>brew install nmap wireshark</code>
        <code>brew install --cask wireshark-chmodbpf</code>
        <p>
          Windows: install Nmap and Wireshark with TShark and the Npcap capture
          driver. Enable the capture permissions appropriate to your account in
          the installer, then restart this app.
        </p>
        <p>
          Tool detection does not prove capture permission. Starting a capture
          checks the actual interface. This app never changes firewall or router
          rules.
        </p>
      </details>
    </section>
  );
}
