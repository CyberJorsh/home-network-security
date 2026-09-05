import { useEffect, useState } from 'react';
import { command, native } from '../api';
import type { Provider } from '../types';
type Auth = {
  busy: boolean;
  signedIn: boolean;
  message: string;
  loginUrl: string | null;
  account: string | null;
  plan: string | null;
  clientVersion: string | null;
};
export default function ProviderAuth({ provider }: { provider: Provider }) {
  const [auth, setAuth] = useState<Auth>();
  const [error, setError] = useState('');
  const [pending, setPending] = useState(false);
  useEffect(() => {
    if (!native) return;
    let alive = true;
    const poll = () =>
      void command<Auth>('auth_status', { provider })
        .then((value) => {
          if (alive) setAuth(value);
        })
        .catch((e) => {
          if (alive) setError(String(e));
        });
    poll();
    const interval = setInterval(poll, 700);
    return () => {
      alive = false;
      clearInterval(interval);
    };
  }, [provider]);
  const action = async (action: string) => {
    setError('');
    setPending(true);
    try {
      await command('auth_action', { provider, action });
      setAuth(await command<Auth>('auth_status', { provider }));
    } catch (e) {
      setError(String(e));
    } finally {
      setPending(false);
    }
  };
  const name = provider === 'chatgpt' ? 'ChatGPT' : 'Grok';
  return (
    <div className="integration-status provider-auth">
      <strong>
        {auth?.signedIn ? `Signed in to ${name}` : `Connect ${name}`}
      </strong>
      <p>
        Sign in through the official{' '}
        {provider === 'chatgpt' ? 'Codex' : 'Grok Build'} client using your
        subscription account. Complete the provider’s browser consent yourself.
      </p>
      <div className="button-row">
        <button
          className="button primary"
          disabled={!native || pending || auth?.busy}
          onClick={() => void action('login')}
        >
          Sign in to {name}
        </button>
        <button
          className="button secondary"
          disabled={!native || pending || auth?.busy}
          onClick={() => void action('check')}
        >
          Check session
        </button>
        <button
          className="link-button"
          disabled={!native || pending || auth?.busy}
          onClick={() => void action('logout')}
        >
          Sign out
        </button>
      </div>
      {auth?.busy && (
        <button
          className="link-button"
          disabled={pending}
          onClick={() => void action('cancel')}
        >
          Cancel
        </button>
      )}
      {auth?.account && (
        <p>
          Account: {auth.account}
          {auth.plan ? ` · ${auth.plan}` : ''}
        </p>
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
      {auth?.clientVersion && <small>Client: {auth.clientVersion}</small>}
      {error && <p role="alert">{error}</p>}
      {!native && (
        <p>
          Sign-in requires the desktop app. This browser is a sample preview.
        </p>
      )}
      <p className="hint">
        Credentials remain in this app’s private provider profile. Checking a
        session sends no model prompt and does not test your remaining
        allowance.
      </p>
      <details>
        <summary>Client setup</summary>
        <p>
          Install the official Codex CLI for ChatGPT or Grok Build CLI for Grok,
          then restart this app. ChatGPT uses Codex subscription allowances.
          Account eligibility and limits are controlled by the provider.
        </p>
        <p>
          Supported protocol checks: Codex 0.153.1 and Grok Build 1.0.18. See
          the repository’s provider guide for installation links.
        </p>
      </details>
    </div>
  );
}
