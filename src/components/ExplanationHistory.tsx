import { useState } from 'react';
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
  const refresh = async () =>
    setItems(await command<Saved[]>('explanation_history'));
  const act = async (name: string, args?: Record<string, unknown>) => {
    setBusy(true);
    try {
      await command(name, args);
      await refresh();
    } catch (e) {
      onError(String(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <details
      className="panel setup-panel"
      onToggle={(e) => {
        if (e.currentTarget.open && native)
          void refresh().catch((e) => onError(String(e)));
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
