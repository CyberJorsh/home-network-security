import { useCallback, useEffect, useRef, useState } from 'react';
import {
  Activity,
  ArrowDownLeft,
  ArrowLeftRight,
  ArrowRight,
  ArrowUpRight,
  Bell,
  Camera,
  Check,
  ChevronRight,
  Copy,
  ExternalLink,
  FileUp,
  HardDrive,
  Laptop,
  LayoutDashboard,
  LockKeyhole,
  Monitor,
  Network,
  Plus,
  Radio,
  RefreshCw,
  Search,
  Shield,
  Smartphone,
  Sparkles,
  Speaker,
  X,
} from 'lucide-react';
import type { Alert, Page, Provider, Snapshot } from './types';
import { acknowledge, command, native, readSnapshot, rename } from './api';
import { buildSummary, bytes, date, filterConversations } from './lib';
import Chart from './components/Chart';
import HostCollection from './components/HostCollection';
import ProviderAuth from './components/ProviderAuth';
import TrafficTable from './components/TrafficTable';

const navigation = [
  { id: 'overview', label: 'Overview', icon: LayoutDashboard },
  { id: 'devices', label: 'Devices', icon: Network },
  { id: 'traffic', label: 'Traffic', icon: Activity },
  { id: 'alerts', label: 'Alerts', icon: Bell },
  { id: 'collection', label: 'Collection', icon: Radio },
  { id: 'assistant', label: 'AI explanations', icon: Sparkles },
] as const;
const titles: Record<Page, [string, string]> = {
  overview: [
    'A little clarity for your network.',
    'See your devices, understand their connections, and know what changed.',
  ],
  devices: [
    'Make yourself familiar.',
    'Give every device a name. Keep the evidence behind each identification.',
  ],
  traffic: [
    'Follow the conversation.',
    'Explore internet traffic and connections between devices at home.',
  ],
  alerts: [
    'A closer look, when it matters.',
    'Observations you can investigate. Every alert starts with evidence.',
  ],
  collection: [
    'Know what you can see.',
    'Connect a collector and understand the limits of each observation point.',
  ],
  assistant: [
    'Understand what you’re seeing.',
    'Prepare a focused summary for your ChatGPT or Grok subscription.',
  ],
};

function DeviceIcon({ name, size = 20 }: { name: string; size?: number }) {
  const Icon = /camera/i.test(name)
    ? Camera
    : /laptop/i.test(name)
      ? Laptop
      : /phone/i.test(name)
        ? Smartphone
        : /speaker/i.test(name)
          ? Speaker
          : /server/i.test(name)
            ? HardDrive
            : Monitor;
  return <Icon size={size} strokeWidth={1.6} />;
}

export default function App() {
  const [page, setPage] = useState<Page>('overview');
  const [mode, setMode] = useState(native ? 'local' : 'demo');
  const [sensor, setSensor] = useState<string | null>(null);
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [error, setError] = useState('');
  const [notice, setNotice] = useState('');
  const [busy, setBusy] = useState(false);
  const [query, setQuery] = useState('');
  const [scope, setScope] = useState('all');
  const [detail, setDetail] = useState<string | null>(null);
  const [name, setName] = useState('');
  const [assistantDevice, setAssistantDevice] = useState('');
  const [assistantAlert, setAssistantAlert] = useState<Alert | undefined>();
  const sequence = useRef(0);
  const loadSnapshot = useCallback(
    async (sourceMode: string, sourceSensor: string | null) => {
      const seq = ++sequence.current;
      try {
        const next = await readSnapshot(sourceMode, sourceSensor);
        if (seq === sequence.current) {
          setSnapshot(next);
          setError('');
        }
      } catch (e) {
        if (seq === sequence.current) setError(String(e));
      }
    },
    [],
  );
  const refresh = useCallback(
    () => loadSnapshot(mode, sensor),
    [loadSnapshot, mode, sensor],
  );
  useEffect(() => {
    void refresh();
    const timer = setInterval(() => {
      void refresh();
    }, 10000);
    return () => {
      clearInterval(timer);
      sequence.current++;
    };
  }, [refresh]);
  useEffect(() => {
    if (!notice) return;
    const timer = setTimeout(() => setNotice(''), 5000);
    return () => clearTimeout(timer);
  }, [notice]);
  const run = async (action: () => Promise<void>) => {
    setBusy(true);
    setError('');
    try {
      await action();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };
  const navigate = (next: Page) => {
    setPage(next);
    setQuery('');
    setDetail(null);
    window.scrollTo({ top: 0 });
  };
  const showDevice = (id: string) => {
    setDetail(id);
    setName(snapshot?.devices.find((d) => d.id === id)?.name ?? '');
  };
  const explain = (id: string, alert?: Alert) => {
    setAssistantDevice(id);
    setAssistantAlert(alert);
    setDetail(null);
    setPage('assistant');
  };
  const importFile = () =>
    run(async () => {
      const imported = await command<{
        count: number;
        sensorId: string;
      } | null>('import_file');
      if (imported !== null) {
        setMode('local');
        setSensor(imported.sensorId);
        setNotice(
          `Imported ${imported.count.toLocaleString()} records into local storage.`,
        );
        await loadSnapshot('local', imported.sensorId);
      }
    });
  useEffect(() => {
    if (!detail) return;
    const previous = document.activeElement;
    const overflow = document.body.style.overflow;
    document.body.style.overflow = 'hidden';
    return () => {
      document.body.style.overflow = overflow;
      if (previous instanceof HTMLElement) previous.focus();
    };
  }, [detail]);
  const isSample = mode === 'demo' || snapshot?.mode === 'demo';
  const currentSensor = snapshot?.sensors.find(
    (s) => s.id === snapshot.selectedSensor,
  );
  const activeAlerts = snapshot?.alerts.filter((a) => !a.acknowledged) ?? [];
  const selectedDevice = snapshot?.devices.find((d) => d.id === detail);

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <a
          className="brand"
          href="#"
          onClick={(e) => {
            e.preventDefault();
            navigate('overview');
          }}
          aria-label="Home Network Security overview"
        >
          <span className="brand-mark">
            <Shield size={22} />
          </span>
          <span>
            home network<span className="brand-sub">SECURITY</span>
          </span>
        </a>
        <div className="workspace-label">YOUR SPACE</div>
        <nav aria-label="Main navigation">
          {navigation.map(({ id, label, icon: Icon }) => (
            <button
              key={id}
              className={`nav-item ${page === id ? 'selected' : ''}`}
              onClick={() => navigate(id)}
              aria-label={label}
              aria-current={page === id ? 'page' : undefined}
            >
              <Icon size={18} strokeWidth={1.6} />
              <span>{label}</span>
              {id === 'alerts' && activeAlerts.length > 0 && (
                <span className="nav-count">{activeAlerts.length}</span>
              )}
            </button>
          ))}
        </nav>
        <div className="sidebar-bottom">
          <div className="local-promise">
            <LockKeyhole size={16} />
            <div>
              Your network. Your data.<small>Stored on your own devices.</small>
            </div>
          </div>
          <div className="version">
            <span className="status-dot" />
            Open source · v0.1 alpha
          </div>
          <a
            href="https://github.com/CyberJorsh/home-network-security"
            target="_blank"
            rel="noreferrer"
          >
            View the project <ExternalLink size={12} />
          </a>
        </div>
      </aside>
      <div className="main-shell">
        <header className="topbar">
          <div className="breadcrumb">
            My network <ChevronRight size={13} />
            <strong>{navigation.find((n) => n.id === page)?.label}</strong>
          </div>
          <div className="top-actions">
            {snapshot && snapshot.sensors.length > 1 && (
              <select
                className="sensor-select"
                aria-label="Observation source"
                value={snapshot.selectedSensor ?? ''}
                onChange={(e) => {
                  setSensor(e.target.value);
                  setDetail(null);
                  setAssistantDevice('');
                }}
              >
                <option value="" disabled>
                  Select source
                </option>
                {snapshot.sensors.map((s) => (
                  <option key={s.id} value={s.id}>
                    {s.name}
                  </option>
                ))}
              </select>
            )}
            <span className={`pill ${isSample ? 'sample' : ''}`}>
              <span className="status-dot" />
              {isSample
                ? 'Sample network'
                : snapshot?.mode === 'collector'
                  ? 'Connected collector'
                  : 'Local data'}
            </span>
            <button
              className="icon-button"
              aria-label="Refresh observations"
              disabled={busy}
              onClick={() => void run(refresh)}
            >
              <RefreshCw size={16} />
            </button>
            <span className="avatar">H</span>
          </div>
        </header>
        <main>
          {error && (
            <div className="banner error" role="alert">
              <strong>Couldn’t complete that action.</strong> {error}{' '}
              {snapshot && 'Displayed observations may be stale.'}
              <button aria-label="Dismiss error" onClick={() => setError('')}>
                <X size={16} />
              </button>
            </div>
          )}
          {notice && (
            <div className="toast" role="status">
              <Check size={16} />
              {notice}
            </div>
          )}
          <div className="page-heading">
            <div>
              <div className="eyebrow">
                {page === 'overview'
                  ? 'THE BIG PICTURE'
                  : navigation.find((n) => n.id === page)?.label.toUpperCase()}
              </div>
              <h1>{titles[page][0]}</h1>
              <p>{titles[page][1]}</p>
            </div>
            {page !== 'assistant' && (
              <button
                className="button primary"
                onClick={() => navigate('collection')}
              >
                <Plus size={16} />
                Connect collector
              </button>
            )}
          </div>
          {isSample && (
            <div className="sample-note">
              <span>
                <Sparkles size={15} />
                <strong>You’re exploring a sample home.</strong> These are
                synthetic observations, not your network.
              </span>
              {native && mode === 'demo' ? (
                <button
                  onClick={() => {
                    setMode('local');
                    setSensor(null);
                  }}
                >
                  Exit sample <ArrowRight size={14} />
                </button>
              ) : (
                <span className="muted">
                  {native ? 'Connected sample collector' : 'Browser preview'}
                </span>
              )}
            </div>
          )}
          {snapshot?.limited && (
            <div className="banner">
              This view includes the latest{' '}
              {snapshot.observationCount.toLocaleString()} of{' '}
              {snapshot.retainedCount.toLocaleString()} retained observations.
              Totals and alerts apply only to this view.
            </div>
          )}
          {!snapshot ? (
            <div className="empty-state">
              <RefreshCw size={24} />
              <h2>Loading observations…</h2>
            </div>
          ) : (
            <>
              {page === 'overview' && (
                <>
                  <div className="stats-grid">
                    <Stat
                      label="Observed devices"
                      value={String(snapshot.devices.length)}
                      caption="In this observation domain"
                      icon={<Network size={18} />}
                    />
                    <Stat
                      label="Downloaded"
                      value={bytes(snapshot.totals.download)}
                      caption="From outside local prefixes"
                      icon={<ArrowDownLeft size={19} />}
                    />
                    <Stat
                      label="Uploaded"
                      value={bytes(snapshot.totals.upload)}
                      caption="To outside local prefixes"
                      icon={<ArrowUpRight size={19} />}
                    />
                    <Stat
                      label="Local transfers"
                      value={bytes(snapshot.totals.localBytes)}
                      caption="Between home devices"
                      icon={<ArrowLeftRight size={18} />}
                    />
                  </div>
                  <section className="panel traffic-panel">
                    <div className="panel-heading">
                      <div>
                        <h2>Network activity</h2>
                        <p>Observed volume · five-minute intervals</p>
                      </div>
                      <span className="quiet-select">
                        {snapshot.timeline.length
                          ? 'Recorded window'
                          : 'Awaiting traffic'}
                      </span>
                    </div>
                    <div className="legend">
                      <span>
                        <i className="green" />
                        Download
                      </span>
                      <span>
                        <i className="blue" />
                        Upload
                      </span>
                      <span>
                        <i className="amber" />
                        Between devices
                      </span>
                    </div>
                    <Chart timeline={snapshot.timeline} />
                  </section>
                  <div className="overview-columns">
                    <section className="panel">
                      <div className="panel-heading">
                        <h2>
                          Around your home{' '}
                          <span className="count">
                            {snapshot.devices.length}
                          </span>
                        </h2>
                        <button
                          className="link-button"
                          onClick={() => navigate('devices')}
                        >
                          All devices <ArrowRight size={14} />
                        </button>
                      </div>
                      <div className="device-list">
                        {snapshot.devices.slice(0, 5).map((d) => (
                          <button
                            className="device-row"
                            key={d.id}
                            onClick={() => showDevice(d.id)}
                          >
                            <span className="device-icon">
                              <DeviceIcon name={d.name} />
                            </span>
                            <span className="device-name">
                              {d.name}
                              <small>{d.addresses[0]}</small>
                            </span>
                            <span className="device-volume">
                              {bytes(d.upload + d.download + d.localBytes)}
                              <small>observed</small>
                            </span>
                            <ChevronRight size={15} />
                          </button>
                        ))}
                        {!snapshot.devices.length && (
                          <Empty
                            title="Meet your network"
                            text="Import observations or connect a collector to see your devices."
                            action="Explore a sample"
                            onAction={() => {
                              setMode('demo');
                              setSensor(null);
                            }}
                          />
                        )}
                      </div>
                    </section>
                    <section className="panel attention-panel">
                      <div className="panel-heading">
                        <h2>Worth a look</h2>
                        <span className="soft-icon">
                          <Bell size={17} />
                        </span>
                      </div>
                      {activeAlerts
                        .filter((a) => a.severity === 'notice')
                        .slice(0, 2)
                        .map((a) => (
                          <div className="attention" key={a.id}>
                            <span className="pill amber-pill">Observation</span>
                            <h3>{a.title}</h3>
                            <p>
                              {
                                snapshot.devices.find(
                                  (d) => d.id === a.deviceId,
                                )?.name
                              }{' '}
                              · {a.detail}
                            </p>
                            <button
                              className="link-button"
                              onClick={() => {
                                navigate('alerts');
                              }}
                            >
                              Review evidence <ArrowRight size={14} />
                            </button>
                          </div>
                        ))}
                      {!activeAlerts.some((a) => a.severity === 'notice') && (
                        <div className="attention">
                          <span className="soft-icon">
                            <Check size={20} />
                          </span>
                          <h3>No volume alerts in this view</h3>
                          <p>
                            This only describes observed traffic. Collection
                            gaps and encrypted activity limit what can be
                            inferred.
                          </p>
                        </div>
                      )}
                      <div className="coverage-foot">
                        <Radio size={16} />
                        <span>
                          Visibility matters
                          <small>
                            Internet:{' '}
                            {currentSensor?.internetCoverage ?? 'not connected'}{' '}
                            · Local:{' '}
                            {currentSensor?.lanCoverage ?? 'not connected'}
                          </small>
                        </span>
                        <button
                          aria-label="Review collection coverage"
                          className="icon-button"
                          onClick={() => navigate('collection')}
                        >
                          <ChevronRight size={16} />
                        </button>
                      </div>
                    </section>
                  </div>
                  <section className="panel">
                    <div className="panel-heading">
                      <div>
                        <h2>Recent conversations</h2>
                        <p>Largest connections in the recorded window</p>
                      </div>
                      <button
                        className="link-button"
                        onClick={() => navigate('traffic')}
                      >
                        Explore traffic <ArrowRight size={14} />
                      </button>
                    </div>
                    <TrafficTable
                      flows={snapshot.conversations.slice(0, 5)}
                      devices={snapshot.devices}
                      onDevice={showDevice}
                    />
                  </section>
                </>
              )}
              {page === 'devices' && (
                <section className="panel">
                  <div className="panel-heading">
                    <h2>
                      Device inventory{' '}
                      <span className="count">{snapshot.devices.length}</span>
                    </h2>
                    <SearchField
                      value={query}
                      onChange={setQuery}
                      placeholder="Find a device or address"
                    />
                  </div>
                  <div className="device-grid">
                    {snapshot.devices
                      .filter((d) =>
                        `${d.name} ${d.addresses.join(' ')} ${d.mac}`
                          .toLowerCase()
                          .includes(query.toLowerCase()),
                      )
                      .map((d) => (
                        <button
                          key={d.id}
                          className="device-card"
                          onClick={() => showDevice(d.id)}
                        >
                          <div className="card-top">
                            <span className="device-icon">
                              <DeviceIcon name={d.name} size={23} />
                            </span>
                            <ChevronRight size={16} />
                          </div>
                          <h3>{d.name}</h3>
                          <p className="mono">{d.addresses[0]}</p>
                          <div className="card-divider" />
                          <div className="mini-metrics">
                            <span>↓ {bytes(d.download)}</span>
                            <span>↑ {bytes(d.upload)}</span>
                          </div>
                          <small>{d.identification}</small>
                        </button>
                      ))}
                  </div>
                  {!snapshot.devices.length && (
                    <Empty
                      title="No devices observed yet"
                      text="Connect a collector or import an Nmap XML file. Discovery and traffic coverage are separate."
                    />
                  )}
                </section>
              )}
              {page === 'traffic' && (
                <section className="panel">
                  <div className="panel-heading">
                    <div className="segmented" aria-label="Traffic scope">
                      {[
                        ['all', 'All traffic'],
                        ['internet', 'Internet'],
                        ['local', 'Between devices'],
                      ].map(([id, label]) => (
                        <button
                          aria-pressed={scope === id}
                          className={scope === id ? 'active' : ''}
                          key={id}
                          onClick={() => setScope(id)}
                        >
                          {label}
                        </button>
                      ))}
                    </div>
                    <SearchField
                      value={query}
                      onChange={setQuery}
                      placeholder="Search device, IP, protocol"
                    />
                  </div>
                  <TrafficTable
                    flows={filterConversations(snapshot, scope, query)}
                    devices={snapshot.devices}
                    onDevice={showDevice}
                  />
                  <div className="table-note">
                    One sensor at a time prevents overlapping observations from
                    inflating totals. Volume includes observed protocol overhead
                    and retransmissions.
                  </div>
                </section>
              )}
              {page === 'alerts' && (
                <section className="panel">
                  <div className="panel-heading">
                    <h2>Observation log</h2>
                    <span className="pill">
                      {activeAlerts.length} unreviewed
                    </span>
                  </div>
                  {snapshot.alerts.map((a) => (
                    <article
                      className={`alert-row ${a.acknowledged ? 'reviewed' : ''}`}
                      key={a.id}
                    >
                      <span className={`alert-symbol ${a.severity}`}>
                        <Bell size={18} />
                      </span>
                      <div className="alert-content">
                        <div className="alert-title">
                          <h3>{a.title}</h3>
                          <span className="pill">
                            {a.acknowledged
                              ? 'Reviewed'
                              : a.severity === 'notice'
                                ? 'Notice'
                                : 'Information'}
                          </span>
                        </div>
                        <p>{a.detail}</p>
                        <small>
                          {
                            snapshot.devices.find((d) => d.id === a.deviceId)
                              ?.name
                          }{' '}
                          · {date(a.timestamp)} · {a.evidence.length} evidence
                          references
                        </small>
                        <details>
                          <summary>View supporting conversations</summary>
                          <TrafficTable
                            flows={snapshot.conversations.filter((c) =>
                              a.evidence.includes(c.id),
                            )}
                            devices={snapshot.devices}
                            onDevice={showDevice}
                          />
                        </details>
                        <div className="alert-actions">
                          <button
                            className="link-button"
                            onClick={() => explain(a.deviceId, a)}
                          >
                            <Sparkles size={14} />
                            Prepare explanation
                          </button>
                          {!a.acknowledged && (
                            <button
                              className="link-button muted"
                              disabled={busy}
                              onClick={() =>
                                void run(async () => {
                                  await acknowledge(mode, a.id);
                                  await refresh();
                                })
                              }
                            >
                              <Check size={14} />
                              Mark reviewed
                            </button>
                          )}
                        </div>
                      </div>
                    </article>
                  ))}
                  {!snapshot.alerts.length && (
                    <Empty
                      title="No observations to review"
                      text="Alerts appear when evidence is available. No alerts does not mean complete protection."
                    />
                  )}
                </section>
              )}
              {page === 'collection' && (
                <Collection
                  snapshot={snapshot}
                  busy={busy}
                  onLocal={(id) => {
                    setMode('local');
                    setSensor(id);
                    void loadSnapshot('local', id);
                  }}
                  onImport={importFile}
                  onSample={() => {
                    setMode('demo');
                    setSensor(null);
                  }}
                  onConnect={(port, token) =>
                    run(async () => {
                      await command('connect_collector', { port, token });
                      setMode('local');
                      setSensor(null);
                      setNotice(
                        'Collector connected. Observations remain on your devices.',
                      );
                      await loadSnapshot('local', null);
                    })
                  }
                  onDisconnect={() =>
                    run(async () => {
                      await command('disconnect_collector');
                      setMode('local');
                      setSensor(null);
                      setNotice('Collector disconnected.');
                      await loadSnapshot('local', null);
                    })
                  }
                  onNetworks={(cidrs) =>
                    run(async () => {
                      await command('configure_networks', { cidrs });
                      await refresh();
                      setNotice('Local prefixes saved.');
                    })
                  }
                />
              )}
              {page === 'assistant' && (
                <Assistant
                  key={`${assistantDevice}:${assistantAlert?.id ?? ''}:${snapshot.selectedSensor}`}
                  snapshot={snapshot}
                  initialDevice={assistantDevice}
                  alert={assistantAlert}
                  onNotice={setNotice}
                  onError={setError}
                />
              )}
              <footer className="page-footer">
                <span>
                  <LockKeyhole size={12} />
                  No automatic cloud uploads
                </span>
                <span>
                  {snapshot.observationCount.toLocaleString()} observations ·{' '}
                  {snapshot.selectedSensor ?? 'No collector'}
                  {snapshot.mode === 'demo' ? ' · synthetic data' : ''}
                </span>
              </footer>
            </>
          )}
        </main>
      </div>
      {selectedDevice && snapshot && (
        <div className="modal-backdrop" onClick={() => setDetail(null)}>
          <section
            className="device-drawer"
            role="dialog"
            aria-modal="true"
            aria-labelledby="device-title"
            onClick={(e) => e.stopPropagation()}
            onKeyDown={(e) => {
              if (e.key === 'Escape') setDetail(null);
              if (e.key === 'Tab') {
                const controls = Array.from(
                  e.currentTarget.querySelectorAll<HTMLElement>(
                    'button:not(:disabled), input, select, textarea, a[href], summary',
                  ),
                );
                const first = controls[0],
                  last = controls[controls.length - 1];
                if (e.shiftKey && document.activeElement === first) {
                  e.preventDefault();
                  last?.focus();
                } else if (!e.shiftKey && document.activeElement === last) {
                  e.preventDefault();
                  first?.focus();
                }
              }
            }}
          >
            <button
              autoFocus
              className="icon-button drawer-close"
              aria-label="Close device details"
              onClick={() => setDetail(null)}
            >
              <X size={20} />
            </button>
            <span className="device-icon large">
              <DeviceIcon name={selectedDevice.name} size={32} />
            </span>
            <div className="eyebrow">DEVICE DETAILS</div>
            <h2 id="device-title">{selectedDevice.name}</h2>
            <p className="muted">{selectedDevice.identification}</p>
            <form
              onSubmit={(e) => {
                e.preventDefault();
                void run(async () => {
                  await rename(mode, selectedDevice.id, name.trim());
                  await refresh();
                  setNotice('Device name saved.');
                });
              }}
            >
              <label htmlFor="device-name">Friendly name</label>
              <div className="inline-input">
                <input
                  id="device-name"
                  required
                  maxLength={100}
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                />
                <button
                  className="button primary"
                  disabled={busy || !name.trim()}
                >
                  Save
                </button>
              </div>
            </form>
            <dl>
              <dt>Observed addresses</dt>
              <dd className="mono">{selectedDevice.addresses.join(', ')}</dd>
              <dt>Observed MAC</dt>
              <dd className="mono">{selectedDevice.mac ?? 'Not available'}</dd>
              <dt>First in retained observations</dt>
              <dd>{date(selectedDevice.firstSeen)}</dd>
              <dt>Last observed</dt>
              <dd>{date(selectedDevice.lastSeen)}</dd>
            </dl>
            <div className="drawer-stats">
              <span>
                Download<strong>{bytes(selectedDevice.download)}</strong>
              </span>
              <span>
                Upload<strong>{bytes(selectedDevice.upload)}</strong>
              </span>
              <span>
                Local<strong>{bytes(selectedDevice.localBytes)}</strong>
              </span>
            </div>
            <p className="hint">
              Names and icons help you organize devices. They do not verify the
              manufacturer, owner, or trustworthiness.
            </p>
            <button
              className="button primary full"
              onClick={() => explain(selectedDevice.id)}
            >
              <Sparkles size={16} />
              Prepare AI explanation
            </button>
            <h3>Connections</h3>
            <TrafficTable
              flows={filterConversations(
                snapshot,
                'all',
                '',
                selectedDevice.id,
              )}
              devices={snapshot.devices}
              onDevice={showDevice}
            />
          </section>
        </div>
      )}
    </div>
  );
}

function Stat({
  label,
  value,
  caption,
  icon,
}: {
  label: string;
  value: string;
  caption: string;
  icon: React.ReactNode;
}) {
  return (
    <div className="stat">
      <div className="stat-label">
        {label}
        <span>{icon}</span>
      </div>
      <div className="stat-value">{value}</div>
      <small>{caption}</small>
    </div>
  );
}
function SearchField({
  value,
  onChange,
  placeholder,
}: {
  value: string;
  onChange: (v: string) => void;
  placeholder: string;
}) {
  return (
    <label className="search-field">
      <Search size={16} />
      <input
        aria-label={placeholder}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
      />
    </label>
  );
}
function Empty({
  title,
  text,
  action,
  onAction,
}: {
  title: string;
  text: string;
  action?: string;
  onAction?: () => void;
}) {
  return (
    <div className="empty-state">
      <Network size={26} />
      <h3>{title}</h3>
      <p>{text}</p>
      {action && (
        <button className="button secondary" onClick={onAction}>
          {action}
          <ArrowRight size={14} />
        </button>
      )}
    </div>
  );
}

function Collection({
  onLocal,
  snapshot,
  busy,
  onImport,
  onSample,
  onConnect,
  onDisconnect,
  onNetworks,
}: {
  snapshot: Snapshot;
  busy: boolean;
  onLocal: (sensor: string) => void;
  onImport: () => void;
  onSample: () => void;
  onConnect: (port: number, token: string) => Promise<void>;
  onDisconnect: () => Promise<void>;
  onNetworks: (cidrs: string) => Promise<void>;
}) {
  const [port, setPort] = useState('9898');
  const [token, setToken] = useState('');
  const [cidrs, setCidrs] = useState(snapshot.networks.join(','));
  return (
    <div className="collection-grid">
      <HostCollection onLocal={onLocal} />
      <section className="panel setup-panel">
        <span className="setup-icon">
          <Radio size={25} />
        </span>
        <h2>Connect your collector</h2>
        <p>
          Read observations from this computer, or an always-on collector
          through an SSH tunnel. The connection token stays in memory.
        </p>
        <form
          onSubmit={(e) => {
            e.preventDefault();
            void onConnect(Number(port), token);
          }}
        >
          <label htmlFor="port">Local connection port</label>
          <input
            id="port"
            type="number"
            min="1"
            max="65535"
            required
            value={port}
            onChange={(e) => setPort(e.target.value)}
          />
          <label htmlFor="token">Collector token</label>
          <input
            id="token"
            type="password"
            required
            minLength={32}
            autoComplete="off"
            value={token}
            onChange={(e) => setToken(e.target.value)}
            placeholder="From your collector token file"
          />
          <button className="button primary full" disabled={busy || !native}>
            Connect collector <ArrowRight size={15} />
          </button>
        </form>
        <button
          className="link-button muted"
          disabled={!native || busy}
          onClick={() => void onDisconnect()}
        >
          Disconnect collector
        </button>
        <details className="setup-details">
          <summary>Collector setup</summary>
          <p>Run the collector on your chosen host:</p>
          <code>hns-collector serve --port 9898</code>
          <p>For another host, forward the API over SSH:</p>
          <code>ssh -N -L 9898:127.0.0.1:9898 user@collector</code>
          <p>
            The server prints the token-file path. Capture is a separate
            explicit command. See the repository’s collector guide for
            permissions and sensor placement.
          </p>
        </details>
      </section>
      <div className="stack">
        <section className="panel setup-panel">
          <div className="panel-heading compact">
            <h2>Collection coverage</h2>
            <span className="pill amber-pill">Unverified</span>
          </div>
          <p>
            A collector only sees traffic that reaches its capture interface. A
            router uplink can miss conversations inside your home.
          </p>
          {snapshot.sensors.map((s) => (
            <div className="sensor-card" key={s.id}>
              <strong>
                <Radio size={15} />
                {s.name}
              </strong>
              <div className="coverage-grid">
                <span>
                  Internet<strong>{s.internetCoverage}</strong>
                </span>
                <span>
                  Between devices<strong>{s.lanCoverage}</strong>
                </span>
              </div>
              <small>{s.notes}</small>
              <dl className="sensor-meta">
                <dt>Status</dt>
                <dd>
                  {s.status === 'collecting' &&
                  (!s.lastSeen || Date.now() / 1000 - s.lastSeen > 30)
                    ? 'No recent observations; check collector'
                    : s.status}
                </dd>
                <dt>Last observation</dt>
                <dd>{s.lastSeen ? date(s.lastSeen) : 'None'}</dd>
                <dt>Packet drops</dt>
                <dd>
                  {s.droppedPackets === null ? 'Unknown' : s.droppedPackets}
                </dd>
              </dl>
            </div>
          ))}
          {!snapshot.sensors.length && (
            <p className="hint">
              No sensors connected. Whole-network visibility has not been
              established.
            </p>
          )}
        </section>
        <section className="panel setup-panel">
          <h2>Bring existing observations</h2>
          <p>
            Import Nmap XML, normalized NDJSON, or PCAP/PCAPNG using a
            separately installed TShark. Files stay local.
          </p>
          <div className="button-row">
            <button
              className="button secondary"
              disabled={!native || busy}
              onClick={onImport}
            >
              <FileUp size={16} />
              Import file
            </button>
            <button className="link-button" onClick={onSample}>
              Explore sample <ArrowRight size={14} />
            </button>
          </div>
        </section>
        <section className="panel setup-panel">
          <h2>Your local networks</h2>
          <p>
            These prefixes determine internet versus local traffic. Include your
            home’s globally routed IPv6 prefix; private ranges alone do not
            cover it.
          </p>
          <form
            onSubmit={(e) => {
              e.preventDefault();
              void onNetworks(cidrs);
            }}
          >
            <label htmlFor="networks">
              Comma-separated CIDRs · local database only
            </label>
            <textarea
              id="networks"
              rows={3}
              value={cidrs}
              onChange={(e) => setCidrs(e.target.value)}
            />
            <button
              className="button secondary"
              disabled={!native || busy || snapshot.mode !== 'local'}
            >
              Save prefixes
            </button>
          </form>
        </section>
      </div>
    </div>
  );
}

function Assistant({
  snapshot,
  initialDevice,
  alert,
  onNotice,
  onError,
}: {
  snapshot: Snapshot;
  initialDevice: string;
  alert?: Alert;
  onNotice: (s: string) => void;
  onError: (s: string) => void;
}) {
  const [provider, setProvider] = useState<Provider>('chatgpt');
  const [deviceId, setDeviceId] = useState(
    initialDevice || snapshot.devices[0]?.id || '',
  );
  const [redact, setRedact] = useState(true);
  const [summary, setSummary] = useState('');
  const [reviewed, setReviewed] = useState(false);
  const prepare = () => {
    try {
      setSummary(
        buildSummary(
          snapshot,
          deviceId,
          redact,
          alert?.deviceId === deviceId ? alert : undefined,
        ),
      );
      setReviewed(false);
    } catch (e) {
      onError(String(e));
    }
  };
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(summary);
      onNotice('Reviewed summary copied. Nothing has been sent.');
    } catch {
      onError('Clipboard access failed. Select and copy the summary manually.');
    }
  };
  return (
    <div className="assistant-grid">
      <section className="panel setup-panel">
        <span className="setup-icon">
          <Sparkles size={25} />
        </span>
        <h2>Your subscription. Your choice.</h2>
        <p>
          Choose the account you already use. Core discovery, traffic views, and
          alerts work without AI.
        </p>
        <div className="provider-options">
          {(['chatgpt', 'grok'] as const).map((p) => (
            <button
              className={`provider ${provider === p ? 'chosen' : ''}`}
              key={p}
              onClick={() => {
                setProvider(p);
                setReviewed(false);
              }}
              aria-pressed={provider === p}
            >
              <span className="provider-logo">
                {p === 'chatgpt' ? <Sparkles size={21} /> : '𝕏'}
              </span>
              <span>
                {p === 'chatgpt' ? 'ChatGPT' : 'Grok'}
                <small>Use your existing subscription</small>
              </span>
              {provider === p && <Check size={17} />}
            </button>
          ))}
        </div>
        <ProviderAuth key={provider} provider={provider} />
        <p className="hint">
          You can test real sign-in here. Explanations still use the reviewed
          copy-and-paste workflow below; embedded model requests remain disabled
          while tool containment and credit controls are verified.
        </p>
        <label htmlFor="summary-device">Device to explain</label>
        <select
          id="summary-device"
          value={deviceId}
          onChange={(e) => {
            setDeviceId(e.target.value);
            setSummary('');
            setReviewed(false);
          }}
        >
          {snapshot.devices.map((d) => (
            <option value={d.id} key={d.id}>
              {d.name}
            </option>
          ))}
        </select>
        <label className="checkbox-label">
          <input
            type="checkbox"
            checked={redact}
            onChange={(e) => {
              setRedact(e.target.checked);
              setSummary('');
              setReviewed(false);
            }}
          />
          <span>Replace names and IP addresses with aliases</span>
        </label>
        <button
          className="button primary full"
          disabled={!deviceId}
          onClick={prepare}
        >
          Prepare summary <ArrowRight size={15} />
        </button>
        <p className="hint">
          At most 12 relevant connections. No packet payloads, MAC addresses,
          unrelated devices, or raw captures.
        </p>
      </section>
      <section className="panel summary-panel">
        <div className="panel-heading">
          <div>
            <h2>Review before sharing</h2>
            <p>You control every word that leaves this app.</p>
          </div>
          <LockKeyhole size={18} />
        </div>
        {summary ? (
          <>
            <label className="sr-only" htmlFor="summary">
              Editable summary
            </label>
            <textarea
              id="summary"
              className="summary-editor"
              spellCheck={false}
              value={summary}
              onChange={(e) => {
                setSummary(e.target.value);
                setReviewed(false);
              }}
            />
            <div className="summary-actions">
              <label className="checkbox-label">
                <input
                  type="checkbox"
                  checked={reviewed}
                  onChange={(e) => setReviewed(e.target.checked)}
                />
                <span>I reviewed this exact summary.</span>
              </label>
              <div className="button-row">
                <button
                  className="button primary"
                  disabled={!reviewed || !summary.trim()}
                  onClick={() => void copy()}
                >
                  <Copy size={15} />
                  Copy reviewed summary
                </button>
                <button
                  className="button secondary"
                  disabled={!reviewed}
                  onClick={() => {
                    if (native) {
                      void command('open_provider', { provider }).catch((e) =>
                        onError(String(e)),
                      );
                    } else {
                      window.open(
                        provider === 'chatgpt'
                          ? 'https://chatgpt.com/'
                          : 'https://grok.com/',
                        '_blank',
                        'noopener,noreferrer',
                      );
                    }
                  }}
                >
                  <ExternalLink size={15} />
                  Open {provider === 'chatgpt' ? 'ChatGPT' : 'Grok'}
                </button>
              </div>
              <small>
                Opening the provider does not send the summary. Paste it there
                when ready. Provider data and allowance policies apply.
              </small>
            </div>
          </>
        ) : (
          <Empty
            title="A focused explanation starts here"
            text="Select a device and prepare a summary. Review or remove any details before sharing them."
          />
        )}
      </section>
    </div>
  );
}
