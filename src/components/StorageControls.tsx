import { useEffect, useState } from 'react';
import { command, native } from '../api';
export default function StorageControls({
  onError,
  onChanged,
}: {
  onError: (s: string) => void;
  onChanged: () => void;
}) {
  const [limit, setLimit] = useState(100000);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState('');
  const [confirming, setConfirming] = useState(false);
  const [confirmation, setConfirmation] = useState('');
  useEffect(() => {
    let alive = true;
    if (native)
      void command<number>('storage_limit')
        .then((n) => {
          if (alive) setLimit(n);
        })
        .catch((e) => {
          if (alive) onError(String(e));
        });
    return () => {
      alive = false;
    };
  }, []);
  const act = async (action: string) => {
    setBusy(true);
    setNotice('');
    try {
      const result = await command(
        action,
        action === 'set_storage_limit'
          ? { limit }
          : action === 'clear_local_data'
            ? { confirmation }
            : undefined,
      );
      if (result !== false) {
        setNotice(
          action === 'set_storage_limit'
            ? 'Storage limit saved. The oldest observations beyond this limit will be removed on the next collection or import.'
            : action === 'export_local_data'
              ? 'Local data exported.'
              : 'Local data deleted.',
        );
        if (action === 'clear_local_data') {
          setConfirming(false);
          setConfirmation('');
          onChanged();
        }
      }
    } catch (e) {
      onError(String(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <details className="panel setup-panel storage-controls">
      <summary>Local history and storage</summary>
      <p>
        Traffic views summarize all retained observations in the selected time
        range. Retention is a record limit, so a busy network fills it sooner.
        Device identity history and saved explanations are kept separately.
      </p>
      <label htmlFor="storage-limit">Maximum stored observations</label>
      <select
        id="storage-limit"
        disabled={!native || busy}
        value={limit}
        onChange={(e) => setLimit(Number(e.target.value))}
      >
        <option value="10000">10,000</option>
        <option value="50000">50,000</option>
        <option value="100000">100,000</option>
      </select>
      <p>
        Reducing this limit removes older traffic on the next collection or
        import. Export first to keep a copy.
      </p>
      <div className="button-row">
        <button
          className="button secondary"
          disabled={!native || busy}
          onClick={() => void act('set_storage_limit')}
        >
          Save storage limit
        </button>
        <button
          className="button secondary"
          disabled={!native || busy}
          onClick={() => void act('export_local_data')}
        >
          Export local data
        </button>
        <button
          className="link-button"
          disabled={!native || busy}
          onClick={() => {
            setConfirming(true);
            setConfirmation('');
          }}
        >
          Delete local data…
        </button>
      </div>
      <p>
        Exports contain observed device identifiers and any saved explanations.
        Store them privately. Deleting local data does not sign out providers or
        delete remote collector data.
      </p>
      {confirming && (
        <section
          className="integration-status"
          aria-label="Confirm local data deletion"
        >
          <h3>Delete this computer’s local data?</h3>
          <p>
            This permanently removes local observations, device names, alerts
            and saved explanations, including when you are viewing a sample.
            Export first to keep a copy.
          </p>
          <label htmlFor="delete-confirmation">
            Type DELETE LOCAL DATA to confirm
          </label>
          <input
            id="delete-confirmation"
            autoComplete="off"
            value={confirmation}
            disabled={busy}
            onChange={(e) => setConfirmation(e.target.value)}
          />
          <div className="button-row">
            <button
              className="button secondary"
              disabled={busy}
              onClick={() => {
                setConfirming(false);
                setConfirmation('');
              }}
            >
              Cancel deletion
            </button>
            <button
              className="button primary"
              disabled={busy || confirmation !== 'DELETE LOCAL DATA'}
              onClick={() => void act('clear_local_data')}
            >
              Permanently delete local data
            </button>
          </div>
        </section>
      )}
      {notice && <p role="status">{notice}</p>}
    </details>
  );
}
