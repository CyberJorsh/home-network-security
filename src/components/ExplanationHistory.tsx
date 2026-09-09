import { useEffect, useRef, useState } from 'react';
import { command, native } from '../api';
import SafeResponse from './SafeResponse';
type Saved = {
  id: string;
  savedAt: number;
  body: { provider: string; model: string; summary: string; text: string };
};
export default function ExplanationHistory({
  canSave,
  onError,
}: {
  canSave: boolean;
  onError: (message: string) => void;
}) {
  const [items, setItems] = useState<Saved[]>([]);
  const [busy, setBusy] = useState(false);
  const lifetime = useRef(0);
  const revision = useRef(0);
  const acting = useRef(false);
  useEffect(
    () => () => {
      lifetime.current++;
      revision.current++;
    },
    [],
  );
  const refresh = async () => {
    const read = ++revision.current;
    try {
      const value = await command<Saved[]>('explanation_history');
      if (read === revision.current) setItems(value);
    } catch (e) {
      if (read === revision.current) onError(String(e));
    }
  };
  const act = async (name: string, args?: Record<string, unknown>) => {
    if (acting.current) return;
    acting.current = true;
    const version = lifetime.current;
    revision.current++;
    setBusy(true);
    try {
      await command(name, args);
      if (version !== lifetime.current) return;
      await refresh();
    } catch (e) {
      if (version === lifetime.current) onError(String(e));
    } finally {
      if (version === lifetime.current) {
        acting.current = false;
        setBusy(false);
      }
    }
  };
  return (
    <details
      className="panel setup-panel"
      onToggle={(e) => {
        if (
          e.target === e.currentTarget &&
          e.currentTarget.open &&
          native &&
          !acting.current
        )
          void refresh();
      }}
    >
      <summary>Saved explanations · on this computer</summary>
      <p>
        History is optional. Save a completed response to keep its exact
        submitted summary and answer. Up to 20 responses are kept; saving
        another removes the oldest.
      </p>
      <button
        className="button secondary"
        disabled={!native || !canSave || busy}
        onClick={() => void act('save_explanation')}
      >
        Save completed explanation
      </button>
      {!items.length && <p>No saved explanations.</p>}
      {items.map((item) => (
        <details key={item.id} className="setup-details">
          <summary>
            {item.body.provider} · {item.body.model} ·{' '}
            {new Date(item.savedAt * 1000).toLocaleString()}
          </summary>
          <details>
            <summary>Submitted summary</summary>
            <pre className="submitted-summary">{item.body.summary}</pre>
          </details>
          <SafeResponse text={item.body.text} />
          <button
            className="link-button"
            disabled={busy}
            onClick={() => void act('delete_explanation', { id: item.id })}
          >
            Delete saved explanation
          </button>
        </details>
      ))}
    </details>
  );
}
