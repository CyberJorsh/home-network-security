import { useEffect, useRef, useState } from 'react';
import { ArrowRight, LockKeyhole, Sparkles } from 'lucide-react';
import { command, native } from '../api';
import { buildSummary } from '../lib';
import type { Alert, Provider, Snapshot } from '../types';
import SafeResponse from './SafeResponse';
import ExplanationHistory from './ExplanationHistory';
import { preferences, savePreferences } from '../preferences';
import ProviderAuth, { type Auth } from './ProviderAuth';
type Model = {
  id: string;
  name: string;
  description: string;
  efforts: string[];
  defaultEffort: string;
  isDefault: boolean;
};
type Catalog = { models: Model[]; allowance: string };
type Output = {
  running: boolean;
  provider: Provider;
  model: string;
  effort?: string;
  summary?: string;
  text: string;
  error: string | null;
  completed: boolean;
};
export default function Assistant({
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
  const [provider, setProvider] = useState<Provider>(
    () => preferences().provider,
  );
  const [auth, setAuth] = useState<Auth>();
  const [catalog, setCatalog] = useState<Catalog>();
  const [model, setModel] = useState('');
  const [effort, setEffort] = useState('');
  const [loading, setLoading] = useState(false);
  const [modelError, setModelError] = useState('');
  const [deviceId, setDeviceId] = useState(
    initialDevice || snapshot.devices[0]?.id || '',
  );
  const [redact, setRedact] = useState(false);
  const [summary, setSummary] = useState('');
  const [prepared, setPrepared] = useState(false);
  const [showReview, setShowReview] = useState(true);
  const [reviewed, setReviewed] = useState(false);
  const [sending, setSending] = useState(false);
  const [output, setOutput] = useState<Output>();
  const generation = useRef(0);
  useEffect(
    () => () => {
      generation.current++;
    },
    [],
  );
  const signedIn = auth?.signedIn;
  useEffect(() => {
    if (!native || !signedIn || auth?.busy || catalog || loading || modelError)
      return;
    const version = generation.current;
    setLoading(true);
    void command<Catalog>('provider_models', { provider })
      .then((value) => {
        if (version !== generation.current) return;
        if (!value.models.length)
          throw new Error(
            'No subscription models are available. Refresh models or check your account.',
          );
        setCatalog(value);
        const saved = preferences().models[provider];
        const initial =
          value.models.find((m) => m.id === saved?.model) ||
          value.models.find((m) => m.isDefault) ||
          value.models[0];
        setModel(initial.id);
        setEffort(
          initial.efforts.includes(saved?.effort || '')
            ? saved!.effort
            : initial.defaultEffort,
        );
        setReviewed(false);
      })
      .catch((e) => {
        if (version === generation.current) setModelError(String(e));
      })
      .finally(() => {
        if (version === generation.current) setLoading(false);
      });
  }, [provider, signedIn, auth?.busy, catalog, loading, modelError]);
  useEffect(() => {
    if (!native) return;
    let alive = true;
    let timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      let running = false;
      try {
        const value = await command<Output>('explanation_status');
        running = value.running;
        if (alive) setOutput(value);
      } catch (e) {
        if (alive) onError(String(e));
      }
      if (alive) timer = setTimeout(() => void poll(), running ? 200 : 1500);
    };
    void poll();
    return () => {
      alive = false;
      clearTimeout(timer);
    };
  }, []);
  useEffect(() => {
    if (model) savePreferences(provider, model, effort);
  }, [provider, model, effort]);
  const selected = catalog?.models.find((m) => m.id === model);
  const busy = sending || output?.running;
  const name = provider === 'chatgpt' ? 'ChatGPT' : 'Grok';
  const compactReview =
    (prepared && !showReview) ||
    (!prepared && Boolean(output?.text || output?.running || output?.error));
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
      setPrepared(true);
      setShowReview(true);
    } catch (e) {
      onError(String(e));
    }
  };
  const send = async () => {
    if (!reviewed || !summary.trim() || busy) return;
    setSending(true);
    setReviewed(false);
    try {
      await command('send_explanation', {
        request: { provider, model, effort, text: summary, reviewed: true },
      });
      setOutput(await command<Output>('explanation_status'));
      setShowReview(false);
      onNotice(`Preparing the reviewed request for ${name}…`);
    } catch (e) {
      onError(String(e));
    } finally {
      setSending(false);
    }
  };
  return (
    <div className={`assistant-grid ${signedIn ? 'account-connected' : ''}`}>
      <section className="panel setup-panel">
        <div className="panel-heading compact">
          <h2>
            {signedIn
              ? 'Explanation settings'
              : 'Your subscription. Your choice.'}
          </h2>
          <Sparkles size={20} />
        </div>
        {!signedIn && (
          <p>
            Choose the account you already use. Discovery and monitoring work
            without AI.
          </p>
        )}
        <div className="provider-switch" aria-label="AI provider">
          {(['chatgpt', 'grok'] as const).map((p) => (
            <button
              key={p}
              className={`button ${provider === p ? 'primary' : 'secondary'}`}
              aria-pressed={provider === p}
              disabled={busy}
              onClick={() => {
                if (p === provider) return;
                generation.current++;
                setLoading(false);
                setModelError('');
                savePreferences(p);
                setProvider(p);
                setAuth(undefined);
                setCatalog(undefined);
                setModel('');
                setEffort('');
                setReviewed(false);
              }}
            >
              {p === 'chatgpt' ? 'ChatGPT' : 'Grok'}
            </button>
          ))}
        </div>
        <ProviderAuth key={provider} provider={provider} onStatus={setAuth} />
        {signedIn && (
          <div className="model-controls">
            <label htmlFor="explanation-model">Model</label>
            <select
              id="explanation-model"
              value={model}
              disabled={busy || loading || !catalog}
              onChange={(e) => {
                setModel(e.target.value);
                setEffort(
                  catalog?.models.find((m) => m.id === e.target.value)
                    ?.defaultEffort || '',
                );
                setReviewed(false);
              }}
            >
              {!catalog && (
                <option value="">
                  {loading ? 'Loading available models…' : 'Models unavailable'}
                </option>
              )}
              {catalog?.models.map((m) => (
                <option key={m.id} value={m.id}>
                  {m.name}
                </option>
              ))}
            </select>
            {selected?.description && (
              <p className="hint">{selected.description}</p>
            )}
            {Boolean(selected?.efforts.length) && (
              <>
                <label htmlFor="reasoning-effort">Reasoning effort</label>
                <select
                  id="reasoning-effort"
                  value={effort}
                  disabled={busy}
                  onChange={(e) => {
                    setEffort(e.target.value);
                    setReviewed(false);
                  }}
                >
                  {selected?.efforts.map((e) => (
                    <option key={e} value={e}>
                      {(
                        {
                          low: 'Low · quicker',
                          medium: 'Medium · balanced',
                          high: 'High · more reasoning',
                          xhigh: 'Extra high',
                          max: 'Maximum',
                          ultra: 'Ultra',
                          minimal: 'Minimal',
                          none: 'None',
                        } as Record<string, string>
                      )[e] || e}
                    </option>
                  ))}
                </select>
                <p className="hint">
                  Higher effort can take longer and use more allowance.
                </p>
              </>
            )}
            <details className="setup-details">
              <summary>Allowance and model availability</summary>
              <p>{catalog?.allowance || modelError}</p>
              <button
                className="link-button"
                disabled={busy || loading || auth?.busy}
                onClick={() => {
                  setReviewed(false);
                  setCatalog(undefined);
                  setModelError('');
                }}
              >
                Refresh models and usage
              </button>
            </details>
          </div>
        )}
        {modelError && <p role="alert">{modelError}</p>}
        <label htmlFor="summary-device">Device to explain</label>
        <select
          id="summary-device"
          value={deviceId}
          disabled={busy}
          onChange={(e) => {
            setDeviceId(e.target.value);
            setSummary('');
            setPrepared(false);
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
            disabled={busy}
            onChange={(e) => {
              setRedact(e.target.checked);
              setSummary('');
              setPrepared(false);
              setReviewed(false);
            }}
          />
          <span>Hide names, addresses, MAC and identifying details</span>
        </label>
        <button
          className="button primary full"
          disabled={!deviceId || busy}
          onClick={prepare}
        >
          Prepare summary <ArrowRight size={15} />
        </button>
        <p className="hint">
          Includes available device details, discovered services and up to 12
          relevant connections. Review or remove anything before sending.
        </p>
      </section>
      <div className="explanation-content">
        <section
          className={`panel summary-panel ${compactReview ? 'compact-review' : ''}`}
        >
          <div className="panel-heading">
            <div>
              <h2>Review before sharing</h2>
              <p>You control every word that leaves this app.</p>
            </div>
            <LockKeyhole size={18} />
          </div>
          {compactReview ? (
            <div className="summary-actions">
              <p>The reviewed request and response appear below.</p>
              <button
                className="button secondary"
                disabled={busy}
                onClick={() => {
                  if (!prepared) prepare();
                  else setShowReview(true);
                  setReviewed(false);
                }}
              >
                {prepared
                  ? 'Edit summary for another send'
                  : 'Prepare another summary'}
              </button>
            </div>
          ) : prepared ? (
            <>
              <label className="sr-only" htmlFor="summary">
                Editable summary
              </label>
              <textarea
                id="summary"
                className="summary-editor"
                spellCheck={false}
                maxLength={65536}
                value={summary}
                disabled={busy}
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
                    disabled={busy}
                    onChange={(e) => setReviewed(e.target.checked)}
                  />
                  <span>I reviewed this exact summary.</span>
                </label>
                <button
                  className="button primary"
                  disabled={
                    !native ||
                    !signedIn ||
                    auth?.busy ||
                    loading ||
                    !selected ||
                    !reviewed ||
                    !summary.trim() ||
                    busy
                  }
                  onClick={() => void send()}
                >
                  {busy ? 'Generating explanation…' : `Send to ${name}`}
                </button>
                <small>
                  The reviewed text is shared with {name}. Provider data and
                  subscription allowance policies apply.
                </small>
              </div>
            </>
          ) : (
            <div className="empty-state">
              <h3>A focused explanation starts here</h3>
              <p>
                Select a device and prepare a summary. Nothing is shared until
                you review and send.
              </p>
            </div>
          )}
        </section>
        {(output?.running || output?.text || output?.error) && (
          <section className="panel response-panel" aria-label="AI response">
            <div className="panel-heading">
              <div>
                <h2>
                  {output.provider === 'grok' ? 'Grok' : 'ChatGPT'} explanation
                </h2>
                <p>
                  {output.model}
                  {output.effort ? ` · ${output.effort} effort` : ''} ·{' '}
                  {output.running
                    ? 'Streaming…'
                    : output.completed
                      ? 'Complete'
                      : 'Stopped'}
                </p>
              </div>
              {output.running && (
                <button
                  className="button secondary"
                  onClick={() =>
                    void command('stop_explanation').catch((e) =>
                      onError(String(e)),
                    )
                  }
                >
                  Stop
                </button>
              )}
            </div>
            {output.summary && (
              <details className="setup-details">
                <summary>Submitted summary</summary>
                <pre className="submitted-summary">{output.summary}</pre>
              </details>
            )}
            <div className="response-text" aria-busy={output.running}>
              <SafeResponse text={output.text || 'Waiting for the provider…'} />
            </div>
            {output.error && (
              <p role="alert">
                {output.error}
                {output.text ? ' The response above is incomplete.' : ''}
              </p>
            )}
            <p className="hint">
              AI explanations can be mistaken. Compare claims with the evidence
              before acting.
            </p>
          </section>
        )}
        <ExplanationHistory
          canSave={Boolean(output?.completed && !output.running)}
          onError={onError}
        />
      </div>
    </div>
  );
}
