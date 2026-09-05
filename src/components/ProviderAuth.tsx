import { useEffect, useState } from 'react';
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
  const [pending, setPending] = useState(false);
  useEffect(() => {
    if (!native) return;
    let alive = true;
    let checking = false;
    const poll = async () => {
      try {
        const value = await command<Auth>('auth_status', { provider });
        if (!alive) return;
        setAuth(value);
        onStatus(value);
        if (!value.message && !value.busy && !checking) {
          checking = true;
          await command('auth_action', { provider, action: 'check' });
        }
      } catch (e) {
        if (alive) setError(String(e));
      }
    };
    void poll();
    const interval = setInterval(() => void poll(), 700);
    return () => {
      alive = false;
      clearInterval(interval);
    };
  }, [provider, onStatus]);
  const action = async (action: string) => {
    setError('');
    setPending(true);
    try {
      await command('auth_action', { provider, action });
      const value = await command<Auth>('auth_status', { provider });
      setAuth(value);
      onStatus(value);
    } catch (e) {
      setError(String(e));
    } finally {
      setPending(false);
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
      {error && <p role="alert">{error}</p>}
      {!native && <p>Provider sessions and sends require the desktop app.</p>}
    </div>
  );
}
