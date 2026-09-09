import { useEffect, useRef, useState } from 'react';
import { command, native } from '../api';
import type { Provider } from '../types';
export type Auth = {
  busy: boolean;
  signedIn: boolean;
  message: string;
  loginUrl: string | null;
  account: string | null;
  plan: string | null;
  clientVersion: string | null;
};
export default function ProviderAuth({
  provider,
  onStatus,
}: {
  provider: Provider;
  onStatus: (auth: Auth) => void;
}) {
  const [auth, setAuth] = useState<Auth>();
  const [error, setError] = useState('');
  const [pollError, setPollError] = useState('');
  const [pending, setPending] = useState(false);
  const lifetime = useRef(0);
  const revision = useRef(0);
  const actionPending = useRef(false);
  const statusCallback = useRef(onStatus);
  statusCallback.current = onStatus;
  useEffect(() => {
    if (!native) return;
    const version = ++lifetime.current;
    actionPending.current = false;
    let checking = false;
    let timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      let delay = 700;
      let read = revision.current;
      try {
        if (actionPending.current) return;
        read = ++revision.current;
        const value = await command<Auth>('auth_status', { provider });
        if (version !== lifetime.current || read !== revision.current) return;
        setAuth(value);
        setPollError('');
        statusCallback.current(value);
        if (value.message && !value.busy) delay = 1500;
        if (!value.message && !value.busy && !checking) {
          checking = true;
          await command('auth_action', { provider, action: 'check' });
        }
      } catch (e) {
        if (version === lifetime.current && read === revision.current)
          setPollError(String(e));
      } finally {
        if (version === lifetime.current)
          timer = setTimeout(() => void poll(), delay);
      }
    };
    void poll();
    return () => {
      lifetime.current++;
      clearTimeout(timer);
    };
  }, [provider]);
  const action = async (action: string) => {
    if (actionPending.current) return;
    const version = lifetime.current;
    actionPending.current = true;
    revision.current++;
    setError('');
    setPollError('');
    setPending(true);
    try {
      await command('auth_action', { provider, action });
      if (version !== lifetime.current) return;
      const read = ++revision.current;
      const value = await command<Auth>('auth_status', { provider });
      if (version !== lifetime.current || read !== revision.current) return;
      setAuth(value);
      statusCallback.current(value);
    } catch (e) {
      if (version === lifetime.current) setError(String(e));
    } finally {
      if (version === lifetime.current) {
        actionPending.current = false;
        setPending(false);
      }
    }
  };
  const name = provider === 'chatgpt' ? 'ChatGPT' : 'Grok';
  const controls = (
    <>
      {auth?.account && (
        <p>
          {auth.account}
          {auth.plan ? ` · ${auth.plan}` : ''}
        </p>
      )}
      <div className="button-row">
        <button
          className="button secondary"
          disabled={!native || pending || auth?.busy}
          onClick={() => void action('check')}
        >
          Check session
        </button>
        {auth?.signedIn && (
          <button
            className="link-button"
            disabled={pending || auth.busy}
            onClick={() => void action('logout')}
          >
            Sign out
          </button>
        )}
      </div>
      {auth?.clientVersion && <small>Client: {auth.clientVersion}</small>}
    </>
  );
  return (
    <div
      className={`provider-auth ${auth?.signedIn ? 'connected' : 'integration-status'}`}
    >
      {auth?.signedIn ? (
        <details>
          <summary>
            <span className="status-dot" />
            {name} connected <span className="muted">Account settings</span>
          </summary>
          {controls}
        </details>
      ) : (
        <>
          <strong>
            {native && (!auth?.message || auth.busy)
              ? `Checking ${name} session…`
              : `Connect ${name}`}
          </strong>
          <p>
            Use your subscription through the official{' '}
            {provider === 'chatgpt' ? 'Codex' : 'Grok Build'} client.
          </p>
          {(!native || (auth?.message && !auth.busy)) && (
            <button
              className="button primary"
              disabled={!native || pending}
              onClick={() => void action('login')}
            >
              Sign in to {name}
            </button>
          )}
          {auth?.busy && (
            <button
              className="link-button"
              disabled={pending}
              onClick={() => void action('cancel')}
            >
              Cancel sign-in check
            </button>
          )}
          {auth?.message && (
            <pre className="tool-output" role="status">
              {auth.message}
            </pre>
          )}
          {auth?.loginUrl && (
            <button
              className="button secondary"
              onClick={() =>
                void command('open_login', { provider }).catch((e) =>
                  setError(String(e)),
                )
              }
            >
              Open sign-in page
            </button>
          )}
          {controls}
          <details>
            <summary>Client setup</summary>
            <p>
              Install Codex 0.153.1 or Grok Build 1.0.18, then restart this app.
              Credentials stay in this app’s private provider profile. ChatGPT
              explanations use Codex subscription allowances.
            </p>
          </details>
        </>
      )}
      {(error || pollError) && <p role="alert">{error || pollError}</p>}
      {!native && <p>Provider sessions and sends require the desktop app.</p>}
    </div>
  );
}
